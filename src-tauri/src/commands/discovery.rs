//! Endpoint management commands (list, create, update).

use aisec_endpoint_metadata::{analyze_endpoint, DiscoveryAnalysisInput};
use aisec_storage::{
    CreateEndpoint, EndpointRepository, ScanRepository, TargetRepository, UpdateEndpoint,
};
use tauri::{AppHandle, State};
use time::OffsetDateTime;
use tracing::{info, instrument};
use url::Url;

use crate::dto::EndpointDto;
use crate::endpoint_pipeline::{analysis_client, target_requires_auth};
use crate::error::{CommandError, CommandResult};
use crate::events::emit_app_data_changed;
use crate::method_heuristic::default_http_method_for_path;
use crate::state::AppState;

fn seed_url_from_descriptor(descriptor_json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(descriptor_json).ok()?;
    for key in ["url", "base_url", "baseUrl"] {
        if let Some(url) = value.get(key).and_then(|v| v.as_str()) {
            if !url.trim().is_empty() {
                return Some(url.trim().to_string());
            }
        }
    }
    None
}

fn target_origin(descriptor_json: &str) -> CommandResult<String> {
    let seed = seed_url_from_descriptor(descriptor_json).ok_or_else(|| {
        CommandError::invalid_input("Target has no URL in its descriptor; add a URL first.")
    })?;
    let parsed = Url::parse(&seed)
        .map_err(|_| CommandError::invalid_input("Target URL is invalid"))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| CommandError::invalid_input("Target URL has no host"))?;
    let mut origin = format!("{}://{}", parsed.scheme(), host);
    if let Some(port) = parsed.port() {
        origin = format!("{origin}:{port}");
    }
    Ok(origin)
}

fn absolute_endpoint_url(origin: &str, path: &str) -> CommandResult<String> {
    let path = path.trim();
    if path.is_empty() {
        return Err(CommandError::invalid_input("path must not be empty"));
    }
    if !path.starts_with('/') {
        return Err(CommandError::invalid_input("path must start with /"));
    }
    Url::parse(origin)
        .and_then(|base| base.join(path))
        .map(|url| url.to_string())
        .map_err(|_| {
            CommandError::invalid_input("could not resolve endpoint URL from target origin and path")
        })
}

fn inferred_method(path_or_url: &str) -> String {
    default_http_method_for_path(path_or_url).to_string()
}

fn normalize_http_method(method: Option<String>) -> CommandResult<String> {
    const ALLOWED: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD"];
    let method = method.unwrap_or_else(|| "GET".into());
    let upper = method.trim().to_ascii_uppercase();
    if upper.is_empty() {
        return Err(CommandError::invalid_input("HTTP method must not be empty"));
    }
    if !ALLOWED.contains(&upper.as_str()) {
        return Err(CommandError::invalid_input(format!(
            "unsupported HTTP method: {upper}"
        )));
    }
    Ok(upper)
}

#[instrument(skip(state))]
pub async fn endpoint_create_op(
    state: &AppState,
    scan_id: String,
    target_id: String,
    method: Option<String>,
    path: String,
) -> CommandResult<EndpointDto> {
    let repos = state.repositories();
    let target = repos.targets().get(&target_id).await.map_err(CommandError::from)?;
    repos
        .scans()
        .get(&scan_id)
        .await
        .map_err(CommandError::from)?;

    let origin = target_origin(&target.descriptor_json)?;
    let url = absolute_endpoint_url(&origin, &path)?;
    let method = match method {
        Some(m) if !m.trim().is_empty() => normalize_http_method(Some(m))?,
        _ => inferred_method(&path),
    };

    let auth_required = target_requires_auth(state, &target.descriptor_json, &url).await;
    let client = analysis_client();
    let metadata = analyze_endpoint(
        &client,
        DiscoveryAnalysisInput {
            endpoint_id: aisec_storage::util::new_id(),
            url: url.clone(),
            method: method.clone(),
            kind: "manual".into(),
            discovery_confidence: 1.0,
            discovery_source: "manual".into(),
            evidence: Some("Manual entry".into()),
            discovered_at: OffsetDateTime::now_utc(),
            auth_required,
        },
    )
    .await;

    let json = metadata
        .to_json()
        .map_err(|e| CommandError::from(aisec_core::AisecError::internal(e.to_string())))?;

    let endpoint = repos
        .endpoints()
        .create(CreateEndpoint {
            scan_id,
            target_id: Some(target.id),
            url,
            kind: "manual".into(),
            method: Some(method),
            confidence: metadata.classification.confidence as f64,
            evidence: Some("Manual entry".into()),
            source_url: Some("manual".into()),
            discovered_at: OffsetDateTime::now_utc(),
            metadata_json: Some(json),
            endpoint_type: Some(metadata.classification.endpoint_type.as_str().into()),
            ai_framework: Some(metadata.classification.ai_framework.clone()),
            risk_score: Some(metadata.risk.score as i64),
            metadata_confidence: Some(metadata.classification.confidence as f64),
            discovery_source: Some("manual".into()),
            auth_required: Some(auth_required),
        })
        .await
        .map_err(CommandError::from)?;

    info!(id = %endpoint.id, url = %endpoint.url, "manual endpoint created with AI metadata");
    Ok(endpoint.into())
}

pub async fn endpoint_list_op(
    state: &AppState,
    scan_id: String,
) -> CommandResult<Vec<EndpointDto>> {
    let endpoints = state
        .repositories()
        .endpoints()
        .list_by_scan(&scan_id)
        .await
        .map_err(CommandError::from)?;
    Ok(endpoints.into_iter().map(EndpointDto::from).collect())
}

#[instrument(skip(state, app))]
pub async fn endpoint_update_op(
    state: &AppState,
    app: &AppHandle,
    endpoint_id: String,
    method: String,
) -> CommandResult<EndpointDto> {
    let method = normalize_http_method(Some(method))?;
    let updated = state
        .repositories()
        .endpoints()
        .update(
            &endpoint_id,
            UpdateEndpoint {
                method: Some(method),
            },
        )
        .await
        .map_err(CommandError::from)?;
    info!(id = %updated.id, method = ?updated.method, "endpoint method updated");
    emit_app_data_changed(app, "endpoint_updated");
    Ok(updated.into())
}

#[tauri::command]
pub async fn endpoint_create(
    state: State<'_, AppState>,
    scan_id: String,
    target_id: String,
    method: Option<String>,
    path: String,
) -> CommandResult<EndpointDto> {
    endpoint_create_op(state.inner(), scan_id, target_id, method, path).await
}

#[tauri::command]
pub async fn endpoint_list(
    state: State<'_, AppState>,
    scan_id: String,
) -> CommandResult<Vec<EndpointDto>> {
    endpoint_list_op(state.inner(), scan_id).await
}

#[tauri::command]
pub async fn endpoint_update(
    app: AppHandle,
    state: State<'_, AppState>,
    endpoint_id: String,
    method: String,
) -> CommandResult<EndpointDto> {
    endpoint_update_op(state.inner(), &app, endpoint_id, method).await
}
