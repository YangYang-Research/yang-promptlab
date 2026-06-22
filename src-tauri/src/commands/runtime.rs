//! Embedded AI runtime IPC — lifecycle, health, benchmark, logs, hardware.

use aisec_runtime::{
    RuntimeBenchmarkResult, RuntimeHardwareProfile, RuntimeHealthReport, RuntimeLogEntry,
    RuntimeStatusSnapshot,
};
use serde::Serialize;
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatusDto {
    pub lifecycle_state: String,
    pub runtime_version: Option<String>,
    pub backend: Option<String>,
    pub platform: Option<String>,
    pub install_path: Option<String>,
    pub installed: bool,
    pub verified: bool,
    pub binary_available: bool,
    pub base_url: String,
    pub model_loaded: bool,
    pub loaded_model_path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHardwareDto {
    pub os: String,
    pub arch: String,
    pub cpu: String,
    pub cpu_cores: usize,
    pub ram_bytes: u64,
    pub gpu_vendor: Option<String>,
    pub gpu_name: Option<String>,
    pub vram_bytes: Option<u64>,
    pub cuda: bool,
    pub metal: bool,
    pub vulkan: bool,
    pub avx2: bool,
    pub detected_at: String,
}

impl From<RuntimeHardwareProfile> for RuntimeHardwareDto {
    fn from(value: RuntimeHardwareProfile) -> Self {
        Self {
            os: value.os,
            arch: value.arch,
            cpu: value.cpu,
            cpu_cores: value.cpu_cores,
            ram_bytes: value.ram_bytes,
            gpu_vendor: value.gpu_vendor,
            gpu_name: value.gpu_name,
            vram_bytes: value.vram_bytes,
            cuda: value.cuda,
            metal: value.metal,
            vulkan: value.vulkan,
            avx2: value.avx2,
            detected_at: value.detected_at.to_string(),
        }
    }
}

fn snapshot_to_dto(snap: RuntimeStatusSnapshot) -> RuntimeStatusDto {
    RuntimeStatusDto {
        lifecycle_state: snap.lifecycle_state,
        runtime_version: snap.runtime_version,
        backend: snap.backend,
        platform: snap.platform,
        install_path: snap.install_path,
        installed: snap.installed,
        verified: snap.verified,
        binary_available: snap.binary_available,
        base_url: snap.base_url,
        model_loaded: snap.model_loaded,
        loaded_model_path: snap.loaded_model_path,
        message: snap.message,
    }
}

pub async fn runtime_status_op(state: &AppState) -> CommandResult<RuntimeStatusDto> {
    let manager = state.runtime_manager().lock().await;
    let snap = manager.status_snapshot_async().await;
    Ok(snapshot_to_dto(snap))
}

#[tauri::command]
pub async fn runtime_status(state: State<'_, AppState>) -> CommandResult<RuntimeStatusDto> {
    runtime_status_op(state.inner()).await
}

#[tauri::command]
pub async fn runtime_install(state: State<'_, AppState>) -> CommandResult<RuntimeStatusDto> {
    let mut manager = state.runtime_manager().lock().await;
    manager
        .install()
        .await
        .map_err(|err| CommandError::from(aisec_core::AisecError::internal(err.to_string())))?;
    let snap = manager.status_snapshot_async().await;
    Ok(snapshot_to_dto(snap))
}

#[tauri::command]
pub async fn runtime_start(state: State<'_, AppState>) -> CommandResult<RuntimeStatusDto> {
    let mut manager = state.runtime_manager().lock().await;
    manager
        .start_runtime()
        .await
        .map_err(map_runtime_err)?;
    let snap = manager.status_snapshot_async().await;
    Ok(snapshot_to_dto(snap))
}

#[tauri::command]
pub async fn runtime_stop(state: State<'_, AppState>) -> CommandResult<RuntimeStatusDto> {
    let mut manager = state.runtime_manager().lock().await;
    manager
        .stop_runtime()
        .await
        .map_err(map_runtime_err)?;
    let snap = manager.status_snapshot_async().await;
    Ok(snapshot_to_dto(snap))
}

#[tauri::command]
pub async fn runtime_restart(state: State<'_, AppState>) -> CommandResult<RuntimeStatusDto> {
    let mut manager = state.runtime_manager().lock().await;
    manager
        .restart_runtime()
        .await
        .map_err(map_runtime_err)?;
    let snap = manager.status_snapshot_async().await;
    Ok(snapshot_to_dto(snap))
}

#[tauri::command]
pub async fn runtime_health(state: State<'_, AppState>) -> CommandResult<RuntimeHealthReport> {
    let mut manager = state.runtime_manager().lock().await;
    manager
        .run_health_check()
        .await
        .map_err(map_runtime_err)
}

#[tauri::command]
pub async fn runtime_benchmark(state: State<'_, AppState>) -> CommandResult<RuntimeBenchmarkResult> {
    let mut manager = state.runtime_manager().lock().await;
    manager
        .run_benchmark()
        .await
        .map_err(map_runtime_err)
}

#[tauri::command]
pub async fn runtime_logs(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> CommandResult<Vec<RuntimeLogEntry>> {
    let manager = state.runtime_manager().lock().await;
    Ok(manager.logs(limit.unwrap_or(100)).await)
}

#[tauri::command]
pub async fn hardware_refresh(state: State<'_, AppState>) -> CommandResult<RuntimeHardwareDto> {
    let mut manager = state.runtime_manager().lock().await;
    let profile = manager
        .refresh_hardware()
        .await
        .map_err(map_runtime_err)?;
    Ok(profile.into())
}

#[tauri::command]
pub async fn runtime_hardware(state: State<'_, AppState>) -> CommandResult<Option<RuntimeHardwareDto>> {
    let manager = state.runtime_manager().lock().await;
    Ok(manager.hardware().cloned().map(RuntimeHardwareDto::from))
}

fn map_runtime_err(err: aisec_runtime::RuntimeError) -> CommandError {
    match err {
        aisec_runtime::RuntimeError::Unavailable => {
            CommandError::invalid_input("AI runtime binary not available — run Install Runtime")
        }
        other => CommandError::from(aisec_core::AisecError::internal(other.to_string())),
    }
}
