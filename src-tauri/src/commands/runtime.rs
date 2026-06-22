//! Embedded AI runtime IPC — lifecycle, health, benchmark, logs, hardware.

use aisec_models::ModelEntry;
use aisec_runtime::{
    RuntimeBenchmarkResult, RuntimeHardwareProfile, RuntimeHealthReport, RuntimeLogEntry,
    RuntimeStatusSnapshot,
};
use serde::Deserialize;
use serde::Serialize;
use tauri::{AppHandle, State};
use time::OffsetDateTime;

use crate::ai_inference_settings::{
    apply_third_party_health_check, format_health_check_timestamp, is_local_model,
    is_third_party_model, load_settings, reconcile_settings, save_settings, settings_to_dto,
    settings_to_dto_with_connectivity_test, AiInferenceRoute,
    AiInferenceSettingsDto,
};
use crate::commands::models::test_third_party_model_connection;
use crate::error::{CommandError, CommandResult};
use crate::events::emit_runtime_install_progress;
use crate::runtime_watch;
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
    pub requires_attention: bool,
    pub last_error: Option<String>,
    pub recommended_runtime: Option<String>,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeConfigurationDto {
    pub mode: String,
    pub status_label: String,
    pub provider: Option<String>,
    pub model_name: Option<String>,
    pub runtime_name: Option<String>,
    pub runtime_version: Option<String>,
    pub connectivity: Option<String>,
    pub last_health_check: Option<String>,
    pub settings: AiInferenceSettingsDto,
    pub runtime_status: RuntimeStatusDto,
}

fn snapshot_to_dto(snap: RuntimeStatusSnapshot, recommended_runtime: Option<String>) -> RuntimeStatusDto {
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
        requires_attention: snap.requires_attention,
        last_error: snap.last_error,
        recommended_runtime,
    }
}

async fn status_dto_for_manager(manager: &aisec_runtime::RuntimeManager) -> RuntimeStatusDto {
    let recommended = manager.recommended_runtime_label();
    let snap = manager.status_snapshot_async().await;
    snapshot_to_dto(snap, recommended)
}

pub async fn runtime_status_op(state: &AppState) -> CommandResult<RuntimeStatusDto> {
    let manager = state.runtime_manager().lock().await;
    Ok(status_dto_for_manager(&manager).await)
}

#[tauri::command]
pub async fn runtime_status(state: State<'_, AppState>) -> CommandResult<RuntimeStatusDto> {
    runtime_status_op(state.inner()).await
}

#[tauri::command]
pub async fn runtime_install(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<RuntimeStatusDto> {
    runtime_repair_inner(app, state.inner()).await
}

#[tauri::command]
pub async fn runtime_repair(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<RuntimeStatusDto> {
    runtime_repair_inner(app, state.inner()).await
}

async fn runtime_repair_inner(app: AppHandle, state: &AppState) -> CommandResult<RuntimeStatusDto> {
    let app_handle = app.clone();
    let mut manager = state.runtime_manager().lock().await;
    manager
        .repair(|step, message, phase| {
            emit_runtime_install_progress(&app_handle, step, message, phase);
        })
        .await
        .map_err(|err| CommandError::from(aisec_core::AisecError::internal(err.to_string())))?;
    Ok(status_dto_for_manager(&manager).await)
}

#[tauri::command]
pub async fn runtime_start(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<RuntimeStatusDto> {
    let mut manager = state.runtime_manager().lock().await;
    manager
        .start_runtime()
        .await
        .map_err(map_runtime_err)?;
    let _ = manager.run_health_check().await;
    runtime_watch::spawn_runtime_watch(app);
    Ok(status_dto_for_manager(&manager).await)
}

#[tauri::command]
pub async fn runtime_stop(state: State<'_, AppState>) -> CommandResult<RuntimeStatusDto> {
    let mut manager = state.runtime_manager().lock().await;
    manager
        .stop_runtime()
        .await
        .map_err(map_runtime_err)?;
    Ok(status_dto_for_manager(&manager).await)
}

#[tauri::command]
pub async fn runtime_restart(state: State<'_, AppState>) -> CommandResult<RuntimeStatusDto> {
    let mut manager = state.runtime_manager().lock().await;
    manager
        .restart_runtime()
        .await
        .map_err(map_runtime_err)?;
    Ok(status_dto_for_manager(&manager).await)
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInferenceRouteRequest {
    pub route: String,
    pub selected_model_id: Option<String>,
}

async fn inference_settings_for_state(state: &AppState) -> CommandResult<AiInferenceSettingsDto> {
    let data_dir = state.data_dir().to_path_buf();
    let models: Vec<ModelEntry> = {
        let manager = state.model_manager().lock().await;
        manager.list_models().into_iter().cloned().collect()
    };

    let mut settings = load_settings(&data_dir).await?;
    if settings.initialized {
        settings = reconcile_settings(settings, &models);
        save_settings(&data_dir, &settings).await?;
    }
    Ok(settings_to_dto(&settings, &models))
}

fn local_status_label(lifecycle: &str) -> String {
    match lifecycle {
        "running" | "busy" => "Running".into(),
        "starting" => "Starting".into(),
        "installed" | "stopped" => "Idle".into(),
        "not_installed" => "Not Installed".into(),
        "downloading" | "installing" => "Installing".into(),
        "failed" => "Failed".into(),
        other => other.replace('_', " "),
    }
}

async fn runtime_configuration_for_state(state: &AppState) -> CommandResult<RuntimeConfigurationDto> {
    let data_dir = state.data_dir().to_path_buf();
    let models: Vec<ModelEntry> = {
        let manager = state.model_manager().lock().await;
        manager.list_models().into_iter().cloned().collect()
    };

    let mut settings = load_settings(&data_dir).await?;
    if settings.initialized {
        settings = reconcile_settings(settings, &models);
        save_settings(&data_dir, &settings).await?;
    }
    let inference = settings_to_dto(&settings, &models);

    let runtime_manager = state.runtime_manager().lock().await;
    let runtime_status = status_dto_for_manager(&runtime_manager).await;
    let last_health = runtime_manager.last_health().cloned();

    let selected_model = settings.selected_model_id.as_ref().and_then(|id| {
        models.iter().find(|m| &m.id == id)
    });

    let (mode, status_label, provider, model_name, runtime_name, runtime_version, connectivity, last_health_check) =
        if !settings.initialized {
            (
                "not_configured".to_string(),
                "Setup Required".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
            )
        } else if settings.route == AiInferenceRoute::ThirdParty {
            let ready = inference.third_party_available;
            (
                "third_party".to_string(),
                if ready { "Ready".to_string() } else { "Setup Required".to_string() },
                selected_model.map(|m| m.display_provider()),
                inference.selected_model_name.clone(),
                None,
                None,
                settings.third_party_connectivity.clone(),
                settings.third_party_last_health_check.clone(),
            )
        } else {
            let lifecycle = runtime_status.lifecycle_state.as_str();
            (
                "local".to_string(),
                local_status_label(lifecycle),
                None,
                inference.selected_model_name.clone().or_else(|| {
                    runtime_status.loaded_model_path.as_ref().map(|p| {
                        std::path::Path::new(p)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or(p)
                            .to_string()
                    })
                }),
                runtime_status.backend.clone().or(Some("llama.cpp".into())),
                runtime_status.runtime_version.clone(),
                last_health.as_ref().map(|h| {
                    if h.endpoint_reachable {
                        format!("Reachable ({} ms)", h.latency_ms)
                    } else if h.process_alive {
                        "Process up".into()
                    } else {
                        "Unreachable".into()
                    }
                }),
                last_health.as_ref().map(|h| h.message.clone()),
            )
        };

    Ok(RuntimeConfigurationDto {
        mode,
        status_label,
        provider,
        model_name,
        runtime_name,
        runtime_version,
        connectivity,
        last_health_check,
        settings: inference,
        runtime_status,
    })
}

#[tauri::command]
pub async fn runtime_configuration(
    state: State<'_, AppState>,
) -> CommandResult<RuntimeConfigurationDto> {
    runtime_configuration_for_state(state.inner()).await
}

#[tauri::command]
pub async fn runtime_inference_settings(
    state: State<'_, AppState>,
) -> CommandResult<AiInferenceSettingsDto> {
    inference_settings_for_state(state.inner()).await
}

#[tauri::command]
pub async fn runtime_set_inference_route(
    state: State<'_, AppState>,
    request: RuntimeInferenceRouteRequest,
) -> CommandResult<AiInferenceSettingsDto> {
    let route = AiInferenceRoute::parse(&request.route).ok_or_else(|| {
        CommandError::invalid_input(format!("unknown inference route: {}", request.route))
    })?;

    if route == AiInferenceRoute::ThirdParty {
        let mut runtime_mgr = state.runtime_manager().lock().await;
        if runtime_mgr.is_runtime_active() {
            let _ = runtime_mgr.stop_runtime().await;
        }
    }

    let data_dir = state.data_dir().to_path_buf();
    let models: Vec<ModelEntry> = {
        let manager = state.model_manager().lock().await;
        manager.list_models().into_iter().cloned().collect()
    };

    let run_connectivity_test = route == AiInferenceRoute::ThirdParty
        && request
            .selected_model_id
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty());

    let mut settings = load_settings(&data_dir).await?;
    settings.route = route;
    settings.initialized = true;

    if let Some(id) = request
        .selected_model_id
        .filter(|value| !value.trim().is_empty())
    {
        let valid = models.iter().any(|model| match route {
            AiInferenceRoute::ThirdParty => is_third_party_model(model) && model.id == id,
            AiInferenceRoute::Local => is_local_model(model) && model.id == id,
        });
        if valid {
            settings.selected_model_id = Some(id);
        }
    }

    settings = reconcile_settings(settings, &models);

    if run_connectivity_test {
        let mut connectivity_test: Option<(bool, String)> = None;
        if let Some(id) = settings.selected_model_id.as_deref() {
            let checked_at = format_health_check_timestamp(OffsetDateTime::now_utc());
            match test_third_party_model_connection(state.inner(), id).await {
                Ok(result) => {
                    apply_third_party_health_check(
                        &mut settings,
                        &checked_at,
                        result.ok,
                        result.latency_ms,
                    );
                    connectivity_test = Some((result.ok, result.message));
                }
                Err(err) => {
                    apply_third_party_health_check(&mut settings, &checked_at, false, 0);
                    connectivity_test = Some((false, err.to_string()));
                }
            }
        }
        save_settings(&data_dir, &settings).await?;
        return Ok(settings_to_dto_with_connectivity_test(
            &settings,
            &models,
            connectivity_test,
        ));
    }

    save_settings(&data_dir, &settings).await?;
    Ok(settings_to_dto(&settings, &models))
}

fn map_runtime_err(err: aisec_runtime::RuntimeError) -> CommandError {
    match err {
        aisec_runtime::RuntimeError::Unavailable => {
            CommandError::invalid_input("AI runtime binary not available — run Install Runtime")
        }
        other => CommandError::from(aisec_core::AisecError::internal(other.to_string())),
    }
}
