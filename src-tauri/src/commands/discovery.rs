//! Discovery execution commands.
//!
//! `discovery_run` executes the real `aisec-discovery` engine against a target's
//! seed URL, persists a scan plus the discovered endpoints into SQLite, and
//! returns the run summary. `endpoint_list` re-reads persisted endpoints for a
//! scan (used to display results and reload after restart).

use aisec_discovery::{DiscoveryConfig, DiscoveryEngine};
use aisec_storage::{
    CreateEndpoint, CreateScan, EndpointRepository, ScanRepository, TargetRepository, UpdateScan,
};
use tauri::State;
use time::OffsetDateTime;
use tracing::{info, instrument, warn};
use url::Url;

use crate::dto::{DiscoveryRunDto, DiscoveryStatsDto, EndpointDto, ScanDto};
use crate::error::{CommandError, CommandResult};
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

fn normalize_http_method(method: Option<String>) -> CommandResult<String> {
    let method = method.unwrap_or_else(|| "GET".into());
    let upper = method.trim().to_ascii_uppercase();
    if upper.is_empty() {
        return Err(CommandError::invalid_input("HTTP method must not be empty"));
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
    let method = normalize_http_method(method)?;

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
        })
        .await
        .map_err(CommandError::from)?;

    info!(id = %endpoint.id, url = %endpoint.url, "manual endpoint created");
    Ok(endpoint.into())
}

#[instrument(skip(state))]
pub async fn discovery_run_op(
    state: &AppState,
    target_id: String,
) -> CommandResult<DiscoveryRunDto> {
    let repos = state.repositories();

    let target = repos.targets().get(&target_id).await.map_err(CommandError::from)?;
    let seed_url = seed_url_from_descriptor(&target.descriptor_json).ok_or_else(|| {
        CommandError::invalid_input("Target has no URL in its descriptor; add a URL first.")
    })?;

    // Create the scan up-front in the running state so it is visible immediately.
    let scan = repos
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
            &scan.id,
            UpdateScan {
                started_at: Some(Some(OffsetDateTime::now_utc())),
                ..Default::default()
            },
        )
        .await;

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

    let inputs: Vec<CreateEndpoint> = report
        .endpoints
        .iter()
        .map(|e| CreateEndpoint {
            scan_id: scan.id.clone(),
            target_id: Some(target.id.clone()),
            url: e.url.clone(),
            kind: e.kind.as_str().to_string(),
            method: e.method.clone(),
            confidence: e.confidence as f64,
            evidence: Some(e.evidence.clone()),
            source_url: e.source_url.clone(),
            discovered_at: e.discovered_at,
        })
        .collect();

    let saved = repos
        .endpoints()
        .create_many(inputs)
        .await
        .map_err(CommandError::from)?;

    let stats = DiscoveryStatsDto {
        pages_fetched: report.stats.pages_fetched as u64,
        pages_failed: report.stats.pages_failed as u64,
        links_extracted: report.stats.links_extracted as u64,
        probes_sent: report.stats.probes_sent as u64,
        duration_ms: report.stats.duration_ms,
        endpoint_count: saved.len() as u64,
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
        endpoints = saved.len(),
        pages = stats.pages_fetched,
        "discovery run completed"
    );

    Ok(DiscoveryRunDto {
        scan: ScanDto::from(updated),
        endpoints: saved.into_iter().map(EndpointDto::from).collect(),
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
    state: State<'_, AppState>,
    target_id: String,
) -> CommandResult<DiscoveryRunDto> {
    discovery_run_op(state.inner(), target_id).await
}

#[tauri::command]
pub async fn endpoint_list(
    state: State<'_, AppState>,
    scan_id: String,
) -> CommandResult<Vec<EndpointDto>> {
    endpoint_list_op(state.inner(), scan_id).await
}
