//! Discovery execution commands.
//!
//! `discovery_run` executes the real `aisec-discovery` engine against a target's
//! seed URL, persists a scan plus the discovered endpoints into SQLite, and
//! returns the run summary. `endpoint_list` re-reads persisted endpoints for a
//! scan (used to display results and reload after restart).

use aisec_discovery::{DiscoveryConfig, DiscoveryEngine};
use aisec_plugin_host::collect_discovery_endpoints;
use aisec_storage::{
    CreateEndpoint, CreateScan, EndpointRepository, ScanRepository, TargetRepository, UpdateEndpoint,
    UpdateScan,
};
use std::collections::HashSet;
use tauri::{AppHandle, State};
use time::OffsetDateTime;
use tracing::{info, instrument, warn};
use url::Url;

use crate::dto::{DiscoveryRunDto, DiscoveryStatsDto, EndpointDto, ScanDto};
use crate::error::{CommandError, CommandResult};
use crate::events::emit_app_data_changed;
use crate::fingerprint_service::{
    fingerprint_endpoint_url, fingerprint_json, should_fingerprint_kind,
};
use crate::method_heuristic::default_http_method_for_path;
use crate::session_auth::resolve_discovery_auth;
use crate::state::AppState;

/// Extract a seed URL from a target's descriptor JSON (`url` or `base_url`).
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

fn method_for_discovered_endpoint(url: &str, reported: Option<&str>) -> String {
    reported
        .map(|m| m.trim().to_ascii_uppercase())
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| inferred_method(url))
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

    let fingerprint_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| CommandError::from(aisec_core::AisecError::internal(e.to_string())))?;
    let fingerprint_json = fingerprint_endpoint_url(
        &fingerprint_client,
        &url,
        Some(method.as_str()),
        "rest_api",
    )
    .await
    .map(|report| fingerprint_json(&report));

    let endpoint = repos
        .endpoints()
        .create(CreateEndpoint {
            scan_id,
            target_id: Some(target.id),
            url,
            kind: "manual".into(),
            method: Some(method),
            confidence: 1.0,
            evidence: Some("Manual entry".into()),
            source_url: Some("manual".into()),
            discovered_at: OffsetDateTime::now_utc(),
            fingerprint_json,
        })
        .await
        .map_err(CommandError::from)?;

    info!(id = %endpoint.id, url = %endpoint.url, "manual endpoint created");
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

    // Run the real discovery engine. allow_private_network is enabled so the
    // desktop tool can scan localhost / internal targets the operator owns.
    // worker_count is pinned to 1: the crawler has a known deadlock with
    // concurrent workers (see crates/aisec-discovery/examples/verify_target.rs
    // and docs/DISCOVERY_VERIFICATION_REPORT.md), so single-worker is the
    // proven-stable setting until that is fixed.
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

    let fingerprint_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| CommandError::from(aisec_core::AisecError::internal(e.to_string())))?;

    let existing_endpoints = repos
        .endpoints()
        .list_by_scan(&scan.id)
        .await
        .map_err(CommandError::from)?;
    let existing_urls: HashSet<String> = existing_endpoints.iter().map(|e| e.url.clone()).collect();

    let mut inputs: Vec<CreateEndpoint> = Vec::with_capacity(report.endpoints.len());
    for e in &report.endpoints {
        if existing_urls.contains(&e.url) {
            continue;
        }
        let method = method_for_discovered_endpoint(&e.url, e.method.as_deref());
        let fingerprint_json = if should_fingerprint_kind(e.kind.as_str()) {
            fingerprint_endpoint_url(
                &fingerprint_client,
                &e.url,
                Some(method.as_str()),
                e.kind.as_str(),
            )
            .await
            .map(|report| fingerprint_json(&report))
        } else {
            None
        };

        inputs.push(CreateEndpoint {
            scan_id: scan.id.clone(),
            target_id: Some(target.id.clone()),
            url: e.url.clone(),
            kind: e.kind.as_str().to_string(),
            method: Some(method),
            confidence: e.confidence as f64,
            evidence: Some(e.evidence.clone()),
            source_url: e.source_url.clone(),
            discovered_at: e.discovered_at,
            fingerprint_json,
        });
    }

    {
        let mut plugin_manager = state.plugin_manager().lock().await;
        if let Ok(plugin_endpoints) = collect_discovery_endpoints(&mut plugin_manager, &seed_url).await
        {
            let mut known: HashSet<String> = existing_urls
                .iter()
                .chain(inputs.iter().map(|e| &e.url))
                .cloned()
                .collect();
            for endpoint in plugin_endpoints {
                if known.contains(&endpoint.url) {
                    continue;
                }
                let method = method_for_discovered_endpoint(
                    &endpoint.url,
                    endpoint.method.as_deref(),
                );
                let fingerprint_json = fingerprint_endpoint_url(
                    &fingerprint_client,
                    &endpoint.url,
                    Some(method.as_str()),
                    if endpoint.kind.is_empty() {
                        "rest_api"
                    } else {
                        endpoint.kind.as_str()
                    },
                )
                .await
                .map(|report| fingerprint_json(&report));
                known.insert(endpoint.url.clone());
                inputs.push(CreateEndpoint {
                    scan_id: scan.id.clone(),
                    target_id: Some(target.id.clone()),
                    url: endpoint.url,
                    kind: if endpoint.kind.is_empty() {
                        "plugin".into()
                    } else {
                        endpoint.kind
                    },
                    method: Some(method),
                    confidence: 0.6,
                    evidence: Some("Discovered by plugin".into()),
                    source_url: Some(seed_url.clone()),
                    discovered_at: OffsetDateTime::now_utc(),
                    fingerprint_json,
                });
            }
        }
    }

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

// ---------------------------------------------------------------------------
// Tauri command wrappers
// ---------------------------------------------------------------------------

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
