//! Embedded llama.cpp runtime IPC — status, lifecycle, GGUF discovery.

use aisec_runtime::{DiscoveredModel, RuntimeProcessState};
use serde::Serialize;
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredModelDto {
    pub name: String,
    pub size_bytes: u64,
    pub modified_at: Option<String>,
    pub digest: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatusDto {
    pub state: String,
    pub binary_path: String,
    pub binary_available: bool,
    pub base_url: String,
    pub healthy: bool,
    pub installed_models: Vec<DiscoveredModelDto>,
    pub message: String,
}

fn model_to_dto(model: DiscoveredModel) -> DiscoveredModelDto {
    DiscoveredModelDto {
        name: model.name,
        size_bytes: model.size_bytes,
        modified_at: model.modified_at,
        digest: model.digest,
    }
}

pub async fn runtime_status_op(state: &AppState) -> CommandResult<RuntimeStatusDto> {
    let mut supervisor = state.runtime_supervisor().lock().await;
    let binary_path = supervisor.binary_path().display().to_string();
    let binary_available = supervisor.binary_available();
    let base_url = supervisor.base_url().to_string();
    let process_state = supervisor.state();
    let healthy = supervisor.check_health().await.unwrap_or(false);

    let installed_models = if healthy {
        supervisor
            .list_installed_models()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(model_to_dto)
            .collect()
    } else {
        Vec::new()
    };

    let message = match process_state {
        RuntimeProcessState::Running if healthy => {
            "embedded runtime is running and healthy".into()
        }
        RuntimeProcessState::Running => "embedded runtime process is up but API is not healthy".into(),
        RuntimeProcessState::Starting => "embedded runtime is starting".into(),
        RuntimeProcessState::Failed => "embedded runtime failed to start".into(),
        RuntimeProcessState::Stopped if !binary_available => {
            "embedded runtime binary not found; place llama-server under runtime/".into()
        }
        RuntimeProcessState::Stopped => "embedded runtime is stopped".into(),
    };

    Ok(RuntimeStatusDto {
        state: process_state.as_str().to_string(),
        binary_path,
        binary_available,
        base_url,
        healthy,
        installed_models,
        message,
    })
}

#[tauri::command]
pub async fn runtime_status(state: State<'_, AppState>) -> CommandResult<RuntimeStatusDto> {
    runtime_status_op(state.inner()).await
}

#[tauri::command]
pub async fn runtime_restart(state: State<'_, AppState>) -> CommandResult<RuntimeStatusDto> {
    let mut supervisor = state.runtime_supervisor().lock().await;
    supervisor
        .restart()
        .await
        .map_err(|err| CommandError::from(aisec_core::AisecError::internal(err.to_string())))?;
    drop(supervisor);
    runtime_status_op(state.inner()).await
}

#[tauri::command]
pub async fn runtime_stop(state: State<'_, AppState>) -> CommandResult<RuntimeStatusDto> {
    let mut supervisor = state.runtime_supervisor().lock().await;
    supervisor
        .stop()
        .await
        .map_err(|err| CommandError::from(aisec_core::AisecError::internal(err.to_string())))?;
    drop(supervisor);
    runtime_status_op(state.inner()).await
}
