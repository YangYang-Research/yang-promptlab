//! Discovery execution — enumerate endpoints then run the AI metadata pipeline.

use std::collections::HashSet;

use aisec_discovery::{DiscoveryConfig, DiscoveryEngine};
use aisec_endpoint_metadata::{analyze_endpoint, DiscoveryAnalysisInput};
use aisec_plugin_host::collect_discovery_endpoints;
use aisec_storage::{
    CreateEndpoint, CreateScan, EndpointRepository, ScanRepository, TargetRepository, UpdateEndpoint,
    UpdateScan,
};
use tauri::{AppHandle, State};
use time::OffsetDateTime;
use tracing::{info, instrument, warn};
use url::Url;

use crate::dto::{DiscoveryRunDto, DiscoveryStatsDto, EndpointDto, ScanDto};
use crate::endpoint_pipeline::{
    analysis_client, analysis_concurrency, build_metadata_for_discovered, target_requires_auth,
    PipelineProgress, DISCOVERY_PIPELINE_PHASES,
};
use crate::error::{CommandError, CommandResult};
use crate::events::{emit_app_data_changed, emit_discovery_progress};
use crate::method_heuristic::default_http_method_for_path;
use crate::session_auth::resolve_discovery_auth;
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

fn plugin_to_discovered(
    endpoint: aisec_plugin_host::PluginDiscoveryEndpoint,
    seed_url: &str,
) -> aisec_discovery::types::DiscoveredEndpoint {
    use aisec_discovery::types::{DiscoveredEndpoint, EndpointKind};
    DiscoveredEndpoint {
        url: endpoint.url,
        kind: EndpointKind::RestApi,
        method: endpoint.method,
        confidence: 0.6,
        evidence: "Discovered by plugin".into(),
        source_url: Some(seed_url.into()),
        discovered_at: OffsetDateTime::now_utc(),
    }
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

#[instrument(skip(state, app))]
pub async fn discovery_run_op(
    state: &AppState,
    app: &AppHandle,
    target_id: String,
    merge_scan_id: Option<String>,
) -> CommandResult<DiscoveryRunDto> {
    let repos = state.repositories();

    let target = repos.targets().get(&target_id).await.map_err(CommandError::from)?;
    let seed_url = seed_url_from_descriptor(&target.descriptor_json).ok_or_else(|| {
        CommandError::invalid_input("Target has no URL in its descriptor; add a URL first.")
    })?;

    let scan = if let Some(existing_id) = merge_scan_id.as_deref() {
        let existing = repos.scans().get(existing_id).await.map_err(CommandError::from)?;
        if existing.target_id.as_deref() != Some(target.id.as_str()) {
            return Err(CommandError::invalid_input(
                "Discovery scan does not belong to this target",
            ));
        }
        let _ = repos
            .scans()
            .update(
                existing_id,
                UpdateScan {
                    status: Some("running".into()),
                    started_at: Some(Some(OffsetDateTime::now_utc())),
                    completed_at: Some(None),
                    ..Default::default()
                },
            )
            .await;
        existing
    } else {
        let created = repos
            .scans()
            .create(CreateScan {
                project_id: target.project_id.clone(),
                target_id: Some(target.id.clone()),
                name: format!("Discovery: {}", target.name),
                status: Some("running".into()),
                playbook_json: None,
            })
            .await
            .map_err(CommandError::from)?;

        let _ = repos
            .scans()
            .update(
                &created.id,
                UpdateScan {
                    started_at: Some(Some(OffsetDateTime::now_utc())),
                    ..Default::default()
                },
            )
            .await;
        created
    };

    emit_discovery_progress(
        app,
        DISCOVERY_PIPELINE_PHASES[0],
        0,
        0,
        0,
    );

    let config = DiscoveryConfig {
        max_depth: 2,
        max_pages: 25,
        worker_count: 1,
        request_timeout: std::time::Duration::from_secs(10),
        allow_private_network: true,
        probe_static_paths: true,
        ..Default::default()
    };

    let engine = DiscoveryEngine::new(config).map_err(CommandError::from)?;
    let session_auth = resolve_discovery_auth(state, &target.descriptor_json, &seed_url).await?;
    let engine = if let Some(auth) = session_auth {
        info!(scan_id = %scan.id, "using authenticated discovery session");
        engine.with_session_auth(auth).map_err(CommandError::from)?
    } else {
        engine
    };

    info!(scan_id = %scan.id, url = %seed_url, "discovery run started");

    let report = match engine.discover(&seed_url).await {
        Ok(report) => report,
        Err(err) => {
            warn!(scan_id = %scan.id, error = %err.client_message(), "discovery run failed");
            let _ = repos
                .scans()
                .update(
                    &scan.id,
                    UpdateScan {
                        status: Some("failed".into()),
                        completed_at: Some(Some(OffsetDateTime::now_utc())),
                        playbook_json: Some(serde_json::json!({ "error": err.client_message() })),
                        ..Default::default()
                    },
                )
                .await;
            return Err(CommandError::from(err));
        }
    };

    let mut discovered = report.endpoints.clone();
  {
        let mut plugin_manager = state.plugin_manager().lock().await;
        if let Ok(plugin_endpoints) = collect_discovery_endpoints(&mut plugin_manager, &seed_url).await
        {
            for endpoint in plugin_endpoints {
                discovered.push(plugin_to_discovered(endpoint, &seed_url));
            }
        }
    }

    let existing_endpoints = repos
        .endpoints()
        .list_by_scan(&scan.id)
        .await
        .map_err(CommandError::from)?;
    let existing_urls: HashSet<String> = existing_endpoints.iter().map(|e| e.url.clone()).collect();
    discovered.retain(|e| !existing_urls.contains(&e.url));

    let auth_required = target_requires_auth(state, &target.descriptor_json, &seed_url).await;
    let client = analysis_client();
    let app_handle = app.clone();
    let scan_id = scan.id.clone();
    let target_id = target.id.clone();

    let inputs = build_metadata_for_discovered(
        &client,
        &discovered,
        &target_id,
        &scan_id,
        auth_required,
        analysis_concurrency(),
        |progress: PipelineProgress| {
            emit_discovery_progress(
                &app_handle,
                &progress.phase,
                progress.processed,
                progress.total,
                progress.elapsed_ms,
            );
        },
    )
    .await;

    let newly_saved = if inputs.is_empty() {
        Vec::new()
    } else {
        repos
            .endpoints()
            .create_many(inputs)
            .await
            .map_err(CommandError::from)?
    };

    let all_endpoints = repos
        .endpoints()
        .list_by_scan(&scan.id)
        .await
        .map_err(CommandError::from)?;

    let stats = DiscoveryStatsDto {
        pages_fetched: report.stats.pages_fetched as u64,
        pages_failed: report.stats.pages_failed as u64,
        links_extracted: report.stats.links_extracted as u64,
        probes_sent: report.stats.probes_sent as u64,
        duration_ms: report.stats.duration_ms,
        endpoint_count: all_endpoints.len() as u64,
        errors: report.errors.clone(),
        phases: DISCOVERY_PIPELINE_PHASES
            .iter()
            .map(|p| (*p).to_string())
            .collect(),
    };

    let updated = repos
        .scans()
        .update(
            &scan.id,
            UpdateScan {
                status: Some("completed".into()),
                completed_at: Some(Some(OffsetDateTime::now_utc())),
                playbook_json: Some(serde_json::json!({
                    "seed_url": seed_url,
                    "pages_fetched": stats.pages_fetched,
                    "pages_failed": stats.pages_failed,
                    "links_extracted": stats.links_extracted,
                    "probes_sent": stats.probes_sent,
                    "duration_ms": stats.duration_ms,
                    "endpoint_count": stats.endpoint_count,
                    "errors": stats.errors,
                    "pipeline": "ai_endpoint_metadata",
                })),
                ..Default::default()
            },
        )
        .await
        .map_err(CommandError::from)?;

    info!(
        scan_id = %scan.id,
        new_endpoints = newly_saved.len(),
        total_endpoints = all_endpoints.len(),
        pages = stats.pages_fetched,
        "discovery run completed"
    );

    emit_app_data_changed(app, "discovery_complete");

    Ok(DiscoveryRunDto {
        scan: ScanDto::from(updated),
        endpoints: all_endpoints.into_iter().map(EndpointDto::from).collect(),
        stats,
    })
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
pub async fn discovery_run(
    app: AppHandle,
    state: State<'_, AppState>,
    target_id: String,
    merge_scan_id: Option<String>,
) -> CommandResult<DiscoveryRunDto> {
    discovery_run_op(state.inner(), &app, target_id, merge_scan_id).await
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
