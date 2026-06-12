//! Scan lifecycle commands — background attack orchestration.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use aisec_attack::AttackCategory;
use aisec_storage::{
    CreateScan, EndpointRepository, FindingRepository, ProjectRepository, Repositories,
    ScanRepository, TargetRepository, UpdateScan,
};
use tauri::State;
use time::OffsetDateTime;
use tracing::{info, instrument, warn};

use crate::commands::attack::run_category_on_endpoint;
use crate::dto::{ScanStartDto, ScanStatusDto};
use crate::error::{CommandError, CommandResult};
use crate::jobs::{ScanJobManager, ScanProgress};
use crate::state::AppState;

fn parse_category(value: &str) -> Option<AttackCategory> {
    AttackCategory::all()
        .iter()
        .copied()
        .find(|cat| cat.as_str() == value)
}

fn progress_to_dto(scan_id: &str, progress: &ScanProgress) -> ScanStatusDto {
    ScanStatusDto {
        scan_id: scan_id.to_string(),
        status: progress.status.clone(),
        progress_percent: progress.progress_percent(),
        completed: progress.completed,
        total: progress.total,
        findings_count: progress.findings,
        current_endpoint: progress.current_endpoint.clone(),
        current_test: progress.current_test.clone(),
        started_at: progress.started_at.clone(),
    }
}

fn progress_from_playbook(playbook_json: Option<&str>) -> Option<ScanProgress> {
    let raw = playbook_json?;
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    serde_json::from_value(value.get("progress")?.clone()).ok()
}

async fn persist_playbook_progress(
    repos: &Repositories,
    scan_id: &str,
    progress: &ScanProgress,
) -> Result<(), aisec_core::AisecError> {
    let scan = repos.scans().get(scan_id).await?;
    let mut playbook = scan
        .playbook_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    if let Some(obj) = playbook.as_object_mut() {
        obj.insert(
            "progress".into(),
            serde_json::to_value(progress).unwrap_or(serde_json::Value::Null),
        );
    }

    repos
        .scans()
        .update(
            scan_id,
            UpdateScan {
                playbook_json: Some(playbook),
                ..Default::default()
            },
        )
        .await?;
    Ok(())
}

async fn wait_if_paused(paused: &AtomicBool, cancel: &AtomicBool) {
    while paused.load(Ordering::Relaxed) {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn run_scan_job(
    db: aisec_storage::Database,
    jobs: ScanJobManager,
    scan_id: String,
    project_id: String,
    target_id: Option<String>,
    endpoint_ids: Vec<String>,
    categories: Vec<AttackCategory>,
    cancel: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    progress: Arc<Mutex<ScanProgress>>,
) {
    let repos = db.repositories();
    let total = (endpoint_ids.len() * categories.len()) as u64;
    let mut findings_total = 0u64;
    let mut had_error = false;

    for endpoint_id in &endpoint_ids {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        wait_if_paused(&paused, &cancel).await;
        if cancel.load(Ordering::Relaxed) {
            break;
        }

        let endpoint = match repos.endpoints().get(endpoint_id).await {
            Ok(endpoint) => endpoint,
            Err(err) => {
                warn!(scan_id = %scan_id, endpoint_id = %endpoint_id, error = %err, "endpoint lookup failed");
                had_error = true;
                continue;
            }
        };

        for category in &categories {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            wait_if_paused(&paused, &cancel).await;
            if cancel.load(Ordering::Relaxed) {
                break;
            }

            {
                if let Ok(mut p) = progress.lock() {
                    p.status = if paused.load(Ordering::Relaxed) {
                        "paused".into()
                    } else {
                        "running".into()
                    };
                    p.current_endpoint = Some(endpoint.url.clone());
                    p.current_test = Some(category.display_name().to_string());
                }
            }

            match run_category_on_endpoint(
                &repos,
                &scan_id,
                &project_id,
                target_id.clone(),
                &endpoint,
                *category,
            )
            .await
            {
                Ok(result) => {
                    findings_total += result.findings.len() as u64;
                    if let Ok(mut p) = progress.lock() {
                        p.completed += 1;
                        p.findings = findings_total;
                    }
                }
                Err(err) => {
                    warn!(
                        scan_id = %scan_id,
                        endpoint_id = %endpoint_id,
                        category = %category.as_str(),
                        error = %err,
                        "attack unit failed"
                    );
                    had_error = true;
                    if let Ok(mut p) = progress.lock() {
                        p.completed += 1;
                    }
                }
            }

            let snapshot = progress.lock().ok().map(|guard| guard.clone());
            if let Some(snapshot) = snapshot {
                let _ = persist_playbook_progress(&repos, &scan_id, &snapshot).await;
            }
        }
    }

    let cancelled = cancel.load(Ordering::Relaxed);
    let final_status = if cancelled {
        "cancelled"
    } else if had_error && findings_total == 0 && total > 0 {
        "failed"
    } else {
        "completed"
    };

    if let Ok(mut p) = progress.lock() {
        p.status = final_status.into();
        p.current_endpoint = None;
        p.current_test = None;
        p.completed = p.completed.min(total);
    }

    let _ = repos
        .scans()
        .update(
            &scan_id,
            UpdateScan {
                status: Some(final_status.into()),
                completed_at: Some(Some(OffsetDateTime::now_utc())),
                ..Default::default()
            },
        )
        .await;

    let snapshot = progress.lock().ok().map(|guard| guard.clone());
    if let Some(snapshot) = snapshot {
        let _ = persist_playbook_progress(&repos, &scan_id, &snapshot).await;
    }

    jobs.remove(&scan_id);
    info!(scan_id = %scan_id, status = final_status, findings = findings_total, "scan job finished");
}

#[instrument(skip(state))]
pub async fn scan_start_op(
    state: &AppState,
    project_id: String,
    target_id: String,
    endpoint_ids: Vec<String>,
    profile: String,
    categories: Vec<String>,
    disabled_tests: Vec<String>,
) -> CommandResult<ScanStartDto> {
    if endpoint_ids.is_empty() {
        return Err(CommandError::invalid_input("At least one endpoint is required"));
    }

    let parsed_categories: Vec<AttackCategory> = categories
        .iter()
        .filter_map(|value| parse_category(value))
        .collect();

    if parsed_categories.is_empty() {
        return Err(CommandError::invalid_input(
            "At least one valid attack category is required",
        ));
    }

    let repos = state.repositories();
    repos
        .projects()
        .get(&project_id)
        .await
        .map_err(CommandError::from)?;
    repos
        .targets()
        .get(&target_id)
        .await
        .map_err(CommandError::from)?;

    for endpoint_id in &endpoint_ids {
        let endpoint = repos
            .endpoints()
            .get(endpoint_id)
            .await
            .map_err(CommandError::from)?;
        if endpoint.target_id.as_deref() != Some(target_id.as_str()) {
            return Err(CommandError::invalid_input(format!(
                "Endpoint {endpoint_id} does not belong to target {target_id}"
            )));
        }
    }

    let scan = repos
        .scans()
        .create(CreateScan {
            project_id: project_id.clone(),
            target_id: Some(target_id.clone()),
            name: format!("Scan ({profile})"),
            status: Some("running".into()),
            playbook_json: Some(serde_json::json!({
                "profile": profile,
                "categories": categories,
                "disabled_tests": disabled_tests,
                "endpoint_ids": endpoint_ids,
            })),
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

    let total = (endpoint_ids.len() * parsed_categories.len()) as u64;
    let cancel = Arc::new(AtomicBool::new(false));
    let paused = Arc::new(AtomicBool::new(false));
    let progress = Arc::new(Mutex::new(ScanProgress::new(total)));
    state.jobs().register(
        scan.id.clone(),
        cancel.clone(),
        paused.clone(),
        progress.clone(),
    );

    let db = state.database().clone();
    let jobs = state.jobs().clone();
    let scan_id = scan.id.clone();

    tauri::async_runtime::spawn(async move {
        run_scan_job(
            db,
            jobs,
            scan_id,
            project_id,
            Some(target_id),
            endpoint_ids,
            parsed_categories,
            cancel,
            paused,
            progress,
        )
        .await;
    });

    info!(scan_id = %scan.id, total_units = total, "scan started in background");
    Ok(ScanStartDto {
        scan_id: scan.id,
    })
}

pub async fn scan_status_op(state: &AppState, scan_id: String) -> CommandResult<ScanStatusDto> {
    if let Some(progress) = state.jobs().progress(&scan_id) {
        return Ok(progress_to_dto(&scan_id, &progress));
    }

    let repos = state.repositories();
    let scan = repos
        .scans()
        .get(&scan_id)
        .await
        .map_err(CommandError::from)?;

    if let Some(progress) = progress_from_playbook(scan.playbook_json.as_deref()) {
        return Ok(progress_to_dto(&scan_id, &progress));
    }

    let findings_count = repos
        .findings()
        .list_by_scan(&scan_id)
        .await
        .map_err(CommandError::from)?
        .len() as u64;

    Ok(ScanStatusDto {
        scan_id,
        status: scan.status.clone(),
        progress_percent: if scan.status == "completed" {
            100.0
        } else {
            0.0
        },
        completed: 0,
        total: 0,
        findings_count,
        current_endpoint: None,
        current_test: None,
        started_at: scan.started_at.map(|dt| {
            dt.format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default()
        }),
    })
}

pub async fn scan_pause_op(state: &AppState, scan_id: String) -> CommandResult<ScanStatusDto> {
    let current = state
        .jobs()
        .progress(&scan_id)
        .ok_or_else(|| CommandError::invalid_input("Scan is not running"))?;
    if current.status == "paused" {
        return scan_status_op(state, scan_id).await;
    }
    if current.status != "running" {
        return Err(CommandError::invalid_input("Scan is not running"));
    }

    if !state.jobs().set_paused(&scan_id, true) {
        return Err(CommandError::invalid_input("Scan is not running"));
    }

    let _ = state
        .repositories()
        .scans()
        .update(
            &scan_id,
            UpdateScan {
                status: Some("paused".into()),
                ..Default::default()
            },
        )
        .await;

    if let Some(progress) = state.jobs().progress(&scan_id) {
        let _ = persist_playbook_progress(&state.repositories(), &scan_id, &progress).await;
    }

    scan_status_op(state, scan_id).await
}

pub async fn scan_resume_op(state: &AppState, scan_id: String) -> CommandResult<ScanStatusDto> {
    let current = state
        .jobs()
        .progress(&scan_id)
        .ok_or_else(|| CommandError::invalid_input("Scan is not paused"))?;
    if current.status != "paused" {
        return Err(CommandError::invalid_input("Scan is not paused"));
    }

    if !state.jobs().set_paused(&scan_id, false) {
        return Err(CommandError::invalid_input("Scan is not paused"));
    }

    let _ = state
        .repositories()
        .scans()
        .update(
            &scan_id,
            UpdateScan {
                status: Some("running".into()),
                ..Default::default()
            },
        )
        .await;

    if let Some(progress) = state.jobs().progress(&scan_id) {
        let _ = persist_playbook_progress(&state.repositories(), &scan_id, &progress).await;
    }

    scan_status_op(state, scan_id).await
}

pub async fn scan_stop_op(state: &AppState, scan_id: String) -> CommandResult<ScanStatusDto> {
    if !state.jobs().request_cancel(&scan_id) {
        return Err(CommandError::invalid_input("Scan is not active"));
    }

    let _ = state
        .repositories()
        .scans()
        .update(
            &scan_id,
            UpdateScan {
                status: Some("cancelled".into()),
                completed_at: Some(Some(OffsetDateTime::now_utc())),
                ..Default::default()
            },
        )
        .await;

    scan_status_op(state, scan_id).await
}

#[tauri::command]
pub async fn scan_start(
    state: State<'_, AppState>,
    project_id: String,
    target_id: String,
    endpoint_ids: Vec<String>,
    profile: String,
    categories: Vec<String>,
    disabled_tests: Vec<String>,
) -> CommandResult<ScanStartDto> {
    scan_start_op(
        state.inner(),
        project_id,
        target_id,
        endpoint_ids,
        profile,
        categories,
        disabled_tests,
    )
    .await
}

#[tauri::command]
pub async fn scan_status(
    state: State<'_, AppState>,
    scan_id: String,
) -> CommandResult<ScanStatusDto> {
    scan_status_op(state.inner(), scan_id).await
}

#[tauri::command]
pub async fn scan_pause(
    state: State<'_, AppState>,
    scan_id: String,
) -> CommandResult<ScanStatusDto> {
    scan_pause_op(state.inner(), scan_id).await
}

#[tauri::command]
pub async fn scan_resume(
    state: State<'_, AppState>,
    scan_id: String,
) -> CommandResult<ScanStatusDto> {
    scan_resume_op(state.inner(), scan_id).await
}

#[tauri::command]
pub async fn scan_stop(state: State<'_, AppState>, scan_id: String) -> CommandResult<ScanStatusDto> {
    scan_stop_op(state.inner(), scan_id).await
}
