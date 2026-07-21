//! Wizard scan draft — persist scan-wizard progress in `scans.playbook_json`.

use aisec_storage::{CreateScan, ProjectRepository, ScanRepository, TargetRepository, UpdateScan};
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{info, instrument};

use crate::dto::ScanDto;
use crate::error::{CommandError, CommandResult};
use crate::events::emit_app_data_changed;
use crate::state::AppState;

pub const WIZARD_SCAN_NAME: &str = "Setup Scan";
pub const WIZARD_SCAN_STATUS: &str = "draft";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanWizardCreateRequest {
    pub project_id: String,
    pub target_id: Option<String>,
    pub wizard: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanWizardSaveRequest {
    pub scan_id: String,
    pub project_id: String,
    pub target_id: Option<String>,
    pub wizard: serde_json::Value,
}

fn parse_playbook(raw: Option<String>) -> serde_json::Value {
    raw.and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

/// Wizard UI state — after `scan_start`, the live copy moves to `wizard_snapshot`.
fn wizard_state_from_playbook(playbook: &serde_json::Value) -> Option<serde_json::Value> {
    playbook
        .get("wizard")
        .cloned()
        .or_else(|| playbook.get("wizard_snapshot").cloned())
}

fn wizard_scan_display_name(target_name: Option<&str>) -> String {
    match target_name {
        Some(name) if !name.trim().is_empty() => format!("{WIZARD_SCAN_NAME} — {name}"),
        _ => WIZARD_SCAN_NAME.into(),
    }
}

#[instrument(skip(state, request))]
pub async fn scan_wizard_create_op(
    state: &AppState,
    request: ScanWizardCreateRequest,
) -> CommandResult<ScanDto> {
    let repos = state.repositories();
    repos
        .projects()
        .get(&request.project_id)
        .await
        .map_err(CommandError::from)?;

    if let Some(ref target_id) = request.target_id {
        let target = repos
            .targets()
            .get(target_id)
            .await
            .map_err(CommandError::from)?;
        if target.project_id != request.project_id {
            return Err(CommandError::invalid_input(
                "target does not belong to this project",
            ));
        }
    }

    let target_label = if let Some(ref target_id) = request.target_id {
        repos
            .targets()
            .get(target_id)
            .await
            .ok()
            .map(|t| t.name)
    } else {
        None
    };

    let scans = repos
        .scans()
        .list_by_project(&request.project_id)
        .await
        .map_err(CommandError::from)?;

    if let Some(existing) = scans
        .into_iter()
        .filter(|scan| scan.status == WIZARD_SCAN_STATUS)
        .filter(|scan| {
            request
                .target_id
                .as_ref()
                .map(|tid| scan.target_id.as_deref() == Some(tid.as_str()))
                .unwrap_or(true)
        })
        .max_by_key(|scan| scan.created_at)
    {
        info!(scan_id = %existing.id, "reusing existing wizard draft scan");
        return Ok(existing.into());
    }

    let scan = repos
        .scans()
        .create(CreateScan {
            project_id: request.project_id,
            target_id: request.target_id,
            name: wizard_scan_display_name(target_label.as_deref()),
            status: Some(WIZARD_SCAN_STATUS.into()),
            playbook_json: Some(serde_json::json!({ "wizard": request.wizard })),
        })
        .await
        .map_err(CommandError::from)?;

    info!(scan_id = %scan.id, "wizard draft scan created");
    Ok(scan.into())
}

#[instrument(skip(state, request))]
pub async fn scan_wizard_save_op(
    state: &AppState,
    request: ScanWizardSaveRequest,
) -> CommandResult<ScanDto> {
    let repos = state.repositories();
    let existing = repos
        .scans()
        .get(&request.scan_id)
        .await
        .map_err(CommandError::from)?;

    if existing.project_id != request.project_id {
        return Err(CommandError::invalid_input("scan project mismatch"));
    }

    if existing.status != WIZARD_SCAN_STATUS {
        return Err(CommandError::invalid_input(
            "only draft wizard scans can be updated",
        ));
    }

    if let Some(ref target_id) = request.target_id {
        let target = repos
            .targets()
            .get(target_id)
            .await
            .map_err(CommandError::from)?;
        if target.project_id != request.project_id {
            return Err(CommandError::invalid_input(
                "target does not belong to this project",
            ));
        }
    }

    let mut playbook = parse_playbook(existing.playbook_json);
    playbook["wizard"] = request.wizard.clone();

    let target_label = if let Some(ref target_id) = request.target_id {
        repos
            .targets()
            .get(target_id)
            .await
            .ok()
            .map(|t| t.name)
    } else {
        None
    };

    let updated = repos
        .scans()
        .update(
            &request.scan_id,
            UpdateScan {
                target_id: Some(request.target_id),
                name: Some(wizard_scan_display_name(target_label.as_deref())),
                playbook_json: Some(playbook),
                ..Default::default()
            },
        )
        .await
        .map_err(CommandError::from)?;

    Ok(updated.into())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanWizardLoadDto {
    pub scan: ScanDto,
    pub wizard: Option<serde_json::Value>,
}

pub async fn scan_wizard_load_op(state: &AppState, scan_id: String) -> CommandResult<ScanWizardLoadDto> {
    let scan = state
        .repositories()
        .scans()
        .get(&scan_id)
        .await
        .map_err(CommandError::from)?;
    let playbook = parse_playbook(scan.playbook_json.clone());
    let wizard = wizard_state_from_playbook(&playbook);
    Ok(ScanWizardLoadDto {
        scan: scan.into(),
        wizard,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wizard_state_prefers_live_wizard_over_snapshot() {
        let playbook = serde_json::json!({
            "wizard": { "version": 7, "currentStep": 5 },
            "wizard_snapshot": { "version": 7, "currentStep": 4 }
        });
        let wizard = wizard_state_from_playbook(&playbook).expect("wizard");
        assert_eq!(wizard["currentStep"], 5);
    }

    #[test]
    fn wizard_state_falls_back_to_snapshot_after_scan_start() {
        let playbook = serde_json::json!({
            "profile": "standard",
            "wizard_snapshot": { "version": 7, "currentStep": 5, "submittedScanId": "scan-1" }
        });
        let wizard = wizard_state_from_playbook(&playbook).expect("wizard");
        assert_eq!(wizard["submittedScanId"], "scan-1");
    }
}

#[tauri::command]
pub async fn scan_wizard_create(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: ScanWizardCreateRequest,
) -> CommandResult<ScanDto> {
    let dto = scan_wizard_create_op(state.inner(), request).await?;
    emit_app_data_changed(&app, "scan_created");
    Ok(dto)
}

#[tauri::command]
pub async fn scan_wizard_save(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: ScanWizardSaveRequest,
) -> CommandResult<ScanDto> {
    let dto = scan_wizard_save_op(state.inner(), request).await?;
    emit_app_data_changed(&app, "scan_updated");
    Ok(dto)
}

#[tauri::command]
pub async fn scan_wizard_load(
    state: State<'_, AppState>,
    scan_id: String,
) -> CommandResult<ScanWizardLoadDto> {
    scan_wizard_load_op(state.inner(), scan_id).await
}
