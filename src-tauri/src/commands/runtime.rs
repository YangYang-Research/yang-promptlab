//! Embedded AI runtime IPC — lifecycle, health, benchmark, logs, hardware.

use aisec_models::{ModelEntry, ModelProvider};
use aisec_runtime::{
    hardware::HardwareDetector, RuntimeBenchmarkResult, RuntimeHardwareProfile,
    RuntimeHealthReport, RuntimeLogEntry, RuntimeLifecycleState, RuntimeStatusSnapshot,
};
use serde::Deserialize;
use serde::Serialize;
use tauri::{AppHandle, State};
use time::OffsetDateTime;

use aisec_inference::config::{AiRuntimeConfiguration, InferenceMode};
use crate::inference_settings::{
    apply_third_party_health_check, config_to_dto, config_to_dto_with_connectivity_test,
    format_health_check_timestamp, is_local_model, is_third_party_model, parse_route,
    reconcile_config, third_party_status_label, AiInferenceSettingsDto,
};
use crate::inference_host::{connectivity_to_judge, open_gateway_session};
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
    pub model_load_in_progress: bool,
    pub settings: AiInferenceSettingsDto,
    pub runtime_status: RuntimeStatusDto,
}

fn local_runtime_display_name(backend: Option<&str>) -> Option<String> {
    match backend.map(str::trim).filter(|value| !value.is_empty()) {
        Some(backend) => Some(format!("llama.cpp - {backend}")),
        None => None,
    }
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
pub async fn runtime_delete(state: State<'_, AppState>) -> CommandResult<RuntimeStatusDto> {
    let mut manager = state.runtime_manager().lock().await;
    manager
        .delete_runtime()
        .await
        .map_err(map_runtime_err)?;
    Ok(status_dto_for_manager(&manager).await)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLoadModelRequest {
    pub model_id: String,
}

#[tauri::command]
pub async fn runtime_load_model(
    app: AppHandle,
    state: State<'_, AppState>,
    request: RuntimeLoadModelRequest,
) -> CommandResult<RuntimeConfigurationDto> {
    let (file_path, model_id) = {
        let manager = state.model_manager().lock().await;
        let entry = manager
            .get_model(&request.model_id)
            .ok_or_else(|| {
                CommandError::invalid_input(format!("model not found: {}", request.model_id))
            })?;
        if entry.provider == ModelProvider::Remote {
            return Err(CommandError::invalid_input(
                "only local GGUF models can be loaded into the runtime",
            ));
        }
        if !entry.file_path.exists() {
            return Err(CommandError::invalid_input(format!(
                "model file missing: {}",
                entry.file_path.display()
            )));
        }
        (entry.file_path.clone(), entry.id.clone())
    };

    {
        let mut inference = state.inference_manager().lock().await;
        let manager = state.model_manager().lock().await;
        let entry = manager
            .get_model(&model_id)
            .ok_or_else(|| CommandError::invalid_input(format!("model not found: {model_id}")))?;
        inference
            .update_from_model(entry, None)
            .await
            .map_err(|e| CommandError::from(aisec_core::AisecError::internal(e.to_string())))?;
    }

    {
        let mut manager = state.runtime_manager().lock().await;
        if !manager.supervisor().binary_available() {
            return Err(map_runtime_err(aisec_runtime::RuntimeError::Unavailable));
        }
        let lifecycle = manager.lifecycle_state();
        if !matches!(
            lifecycle,
            RuntimeLifecycleState::Running
                | RuntimeLifecycleState::Starting
                | RuntimeLifecycleState::Busy
        ) {
            return Err(CommandError::invalid_input(
                "Start Runtime before loading a model",
            ));
        }
        if !manager.is_same_model_loaded_at(&file_path).await {
            load_model_with_loading_cache(
                state.inner(),
                &mut manager,
                &file_path,
                &model_id,
            )
                .await
                .map_err(map_runtime_err)?;
        } else {
            let _ = manager.run_health_check().await;
        }
    }

    runtime_watch::spawn_runtime_watch(app);
    runtime_configuration_for_state(state.inner()).await
}

#[tauri::command]
pub async fn runtime_unload_model(
    state: State<'_, AppState>,
) -> CommandResult<RuntimeConfigurationDto> {
    let mut manager = state.runtime_manager().lock().await;
    manager
        .unload_loaded_model()
        .await
        .map_err(map_runtime_err)?;
    runtime_configuration_for_state(state.inner()).await
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
    if let Ok(manager) = state.runtime_manager().try_lock() {
        return Ok(manager.logs(limit.unwrap_or(100)).await);
    }
    Ok(Vec::new())
}

#[tauri::command]
pub async fn hardware_refresh(state: State<'_, AppState>) -> CommandResult<RuntimeHardwareDto> {
    if let Ok(mut manager) = state.runtime_manager().try_lock() {
        let profile = manager
            .refresh_hardware()
            .await
            .map_err(map_runtime_err)?;
        return Ok(profile.into());
    }

    let profile = HardwareDetector::new(state.data_dir())
        .detect_and_persist()
        .await
        .map_err(map_runtime_err)?;
    Ok(profile.into())
}

#[tauri::command]
pub async fn runtime_hardware(state: State<'_, AppState>) -> CommandResult<Option<RuntimeHardwareDto>> {
    if let Ok(manager) = state.runtime_manager().try_lock() {
        if let Some(profile) = manager.hardware() {
            return Ok(Some(profile.clone().into()));
        }
    }
    let profile = HardwareDetector::new(state.data_dir())
        .load()
        .await
        .map_err(map_runtime_err)?;
    Ok(profile.map(RuntimeHardwareDto::from))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInferenceRouteRequest {
    pub route: String,
    pub selected_model_id: Option<String>,
}

async fn run_third_party_connectivity_test_for_config(
    state: &AppState,
    config: &mut AiRuntimeConfiguration,
    model_id: &str,
) -> (bool, String) {
    let checked_at = format_health_check_timestamp(OffsetDateTime::now_utc());
    match test_third_party_model_connection(state, model_id).await {
        Ok(result) => {
            apply_third_party_health_check(config, &checked_at, result.ok, result.latency_ms);
            (result.ok, result.message)
        }
        Err(err) => {
            apply_third_party_health_check(config, &checked_at, false, 0);
            (false, err.to_string())
        }
    }
}

async fn reconcile_inference_config(
    state: &AppState,
    models: &[ModelEntry],
) -> CommandResult<(AiRuntimeConfiguration, Option<(bool, String)>)> {
    let mut inference = state.inference_manager().lock().await;
    let loaded = inference.config().clone();
    if !loaded.initialized {
        return Ok((loaded, None));
    }

    let mut config = loaded.clone();
    let previous_selected = config.selected_model_id.clone();
    config = reconcile_config(config, models);

    let mut connectivity_test = None;
    if config.mode == InferenceMode::ThirdParty
        && config.selected_model_id != previous_selected
        && config.selected_model_id.is_some()
    {
        if let Some(id) = config.selected_model_id.clone() {
            let result =
                run_third_party_connectivity_test_for_config(state, &mut config, &id).await;
            connectivity_test = Some(result);
        }
    }

    if config != loaded {
        *inference.config_mut() = config.clone();
        inference
            .save()
            .await
            .map_err(|e| CommandError::from(aisec_core::AisecError::internal(e.to_string())))?;
    }
    Ok((config, connectivity_test))
}

async fn inference_settings_for_state(state: &AppState) -> CommandResult<AiInferenceSettingsDto> {
    let models: Vec<ModelEntry> = {
        let manager = state.model_manager().lock().await;
        manager.list_models().into_iter().cloned().collect()
    };

    let (config, connectivity_test) = reconcile_inference_config(state, &models).await?;
    Ok(config_to_dto_with_connectivity_test(
        &config,
        &models,
        connectivity_test,
    ))
}

fn local_status_label(
    lifecycle: &str,
    binary_available: bool,
    manifest_installed: bool,
    model_loaded: bool,
) -> String {
    if manifest_installed && !binary_available {
        return "Repair Required".into();
    }
    match lifecycle {
        "running" | "busy" if model_loaded => "Running".into(),
        "running" | "busy" => "Ready".into(),
        "starting" if !model_loaded => "Loading model".into(),
        "starting" => "Starting".into(),
        "stopping" => "Stopping".into(),
        "stopped" => "Stopped".into(),
        "installed" => "Idle".into(),
        "not_installed" => "Not Installed".into(),
        "downloading" | "installing" => "Installing".into(),
        "failed" => "Failed".into(),
        other => other.replace('_', " "),
    }
}

async fn store_runtime_configuration_cache(state: &AppState, dto: &RuntimeConfigurationDto) {
    *state.runtime_config_cache().lock().await = Some(dto.clone());
}

async fn assemble_runtime_configuration(
    models: &[ModelEntry],
    config: &AiRuntimeConfiguration,
    inference: &AiInferenceSettingsDto,
    runtime_manager: &aisec_runtime::RuntimeManager,
) -> RuntimeConfigurationDto {
    let runtime_status = status_dto_for_manager(runtime_manager).await;
    let last_health = runtime_manager.last_health().cloned();

    let selected_model = config.selected_model_id.as_ref().and_then(|id| {
        models.iter().find(|m| &m.id == id)
    });

    let (mode, status_label, provider, model_name, runtime_name, runtime_version, connectivity, last_health_check) =
        if !config.initialized {
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
        } else if config.mode == InferenceMode::ThirdParty {
            (
                "third_party".to_string(),
                third_party_status_label(
                    config,
                    inference.third_party_available,
                    selected_model,
                ),
                selected_model.map(|m| m.display_provider()),
                inference.selected_model_name.clone(),
                None,
                None,
                if config.health.message.is_empty() {
                    None
                } else {
                    Some(config.health.message.clone())
                },
                config.health.checked_at.clone(),
            )
        } else {
            let lifecycle = runtime_status.lifecycle_state.as_str();
            let manifest_installed = runtime_manager
                .manifest()
                .is_some_and(|m| m.installed);
            (
                "local".to_string(),
                local_status_label(
                    lifecycle,
                    runtime_status.binary_available,
                    manifest_installed,
                    runtime_status.model_loaded,
                ),
                None,
                if runtime_status.model_loaded {
                    runtime_status.loaded_model_path.as_ref().map(|p| {
                        std::path::Path::new(p)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or(p)
                            .to_string()
                    })
                } else {
                    None
                },
                local_runtime_display_name(runtime_status.backend.as_deref()),
                runtime_status.runtime_version.clone(),
                last_health.as_ref().map(|h| {
                    if h.endpoint_reachable {
                        format!("Reachable ({} ms)", h.latency_ms)
                    } else if h.model_loaded {
                        "Unreachable".into()
                    } else if matches!(lifecycle, "running" | "busy" | "starting") {
                        "Offline — load a model".into()
                    } else {
                        "Not checked".into()
                    }
                }),
                last_health.as_ref().and_then(|h| {
                    if h.message.is_empty() {
                        None
                    } else {
                        Some(h.message.clone())
                    }
                }),
            )
        };

    RuntimeConfigurationDto {
        mode,
        status_label,
        provider,
        model_name,
        runtime_name,
        runtime_version,
        connectivity,
        last_health_check,
        model_load_in_progress: false,
        settings: inference.clone(),
        runtime_status,
    }
}

fn fallback_runtime_status_when_busy() -> RuntimeStatusDto {
    RuntimeStatusDto {
        lifecycle_state: "starting".into(),
        runtime_version: None,
        backend: None,
        platform: None,
        install_path: None,
        installed: true,
        verified: false,
        binary_available: true,
        base_url: "embedded".into(),
        model_loaded: false,
        loaded_model_path: None,
        message: "Loading GGUF model via embedded libllama — large models may take several minutes on CPU".into(),
        requires_attention: false,
        last_error: None,
        recommended_runtime: None,
    }
}

async fn assemble_runtime_configuration_busy_fallback(
    models: &[ModelEntry],
    config: &AiRuntimeConfiguration,
    inference: &AiInferenceSettingsDto,
) -> RuntimeConfigurationDto {
    let selected_model = config.selected_model_id.as_ref().and_then(|id| {
        models.iter().find(|m| &m.id == id)
    });
    let runtime_status = fallback_runtime_status_when_busy();

    let (mode, status_label, provider, model_name, runtime_name, runtime_version, connectivity, last_health_check) =
        if !config.initialized {
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
        } else if config.mode == InferenceMode::ThirdParty {
            (
                "third_party".to_string(),
                third_party_status_label(
                    config,
                    inference.third_party_available,
                    selected_model,
                ),
                selected_model.map(|m| m.display_provider()),
                inference.selected_model_name.clone(),
                None,
                None,
                if config.health.message.is_empty() {
                    None
                } else {
                    Some(config.health.message.clone())
                },
                config.health.checked_at.clone(),
            )
        } else {
            (
                "local".to_string(),
                "Loading model".into(),
                None,
                inference.selected_model_name.clone(),
                local_runtime_display_name(runtime_status.backend.as_deref()),
                None,
                Some("Offline — load a model".into()),
                None,
            )
        };

    RuntimeConfigurationDto {
        mode,
        status_label,
        provider,
        model_name,
        runtime_name,
        runtime_version,
        connectivity,
        last_health_check,
        model_load_in_progress: false,
        settings: inference.clone(),
        runtime_status,
    }
}

async fn runtime_model_loading_id(state: &AppState) -> Option<String> {
    state.runtime_model_loading_id().lock().await.clone()
}

pub(crate) async fn set_runtime_model_loading(state: &AppState, model_id: Option<String>) {
    *state.runtime_model_loading_id().lock().await = model_id;
}

fn apply_model_loading_overlay(dto: &mut RuntimeConfigurationDto, loading_model_id: Option<&str>) {
    let Some(model_id) = loading_model_id else {
        return;
    };
    dto.model_load_in_progress = true;
    dto.status_label = "Loading model".into();
    dto.runtime_status.lifecycle_state = "starting".into();
    dto.runtime_status.model_loaded = false;
    dto.runtime_status.loaded_model_path = None;
    dto.runtime_status.message =
        "Loading GGUF model via embedded libllama — large models may take several minutes on CPU".into();
    if dto.mode == "not_configured" {
        dto.mode = "local".into();
    }
    dto.settings.selected_model_id = Some(model_id.to_string());
}

/// Reserved for startup auto-resume; loading UI is driven by `runtime_model_loading_id`.
pub(crate) async fn prime_loading_configuration_cache(_state: &AppState, _model_id: &str) {}

/// Refresh the configuration cache while the runtime manager lock is already held.
pub(crate) async fn prime_runtime_configuration_cache(
    state: &AppState,
    runtime_manager: &aisec_runtime::RuntimeManager,
) {
    let models: Vec<ModelEntry> = {
        let manager = state.model_manager().lock().await;
        manager.list_models().into_iter().cloned().collect()
    };
    let config = {
        let inference = state.inference_manager().lock().await;
        inference.config().clone()
    };
    let inference_dto = config_to_dto(&config, &models);
    let dto = assemble_runtime_configuration(&models, &config, &inference_dto, runtime_manager).await;
    store_runtime_configuration_cache(state, &dto).await;
}

/// Shared model-load path for manual IPC and startup auto-resume.
pub(crate) async fn load_model_with_loading_cache(
    state: &AppState,
    manager: &mut aisec_runtime::RuntimeManager,
    file_path: &std::path::Path,
    model_id: &str,
) -> Result<(), aisec_runtime::RuntimeError> {
    set_runtime_model_loading(state, Some(model_id.to_string())).await;

    let result = async {
        if !manager.is_model_loaded_at(file_path).await {
            manager.on_model_load_started();
            prime_runtime_configuration_cache(state, manager).await;
        }
        manager.load_model_at_path(file_path).await
    }
    .await;

    if result.is_ok() {
        let _ = manager.run_health_check().await;
    }
    set_runtime_model_loading(state, None).await;
    prime_runtime_configuration_cache(state, manager).await;
    result
}

async fn runtime_configuration_for_state(state: &AppState) -> CommandResult<RuntimeConfigurationDto> {
    let models: Vec<ModelEntry> = {
        let manager = state.model_manager().lock().await;
        manager.list_models().into_iter().cloned().collect()
    };

    let (config, _) = reconcile_inference_config(state, &models).await?;
    let inference = config_to_dto(&config, &models);
    let loading_model_id = runtime_model_loading_id(state).await;

    let base = if let Ok(runtime_manager) = state.runtime_manager().try_lock() {
        let dto =
            assemble_runtime_configuration(&models, &config, &inference, &runtime_manager).await;
        store_runtime_configuration_cache(state, &dto).await;
        dto
    } else if let Some(cached) = state.runtime_config_cache().lock().await.clone() {
        cached
    } else {
        assemble_runtime_configuration_busy_fallback(&models, &config, &inference).await
    };

    let mut response = base;
    apply_model_loading_overlay(&mut response, loading_model_id.as_deref());
    Ok(response)
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
    let route = parse_route(&request.route).ok_or_else(|| {
        CommandError::invalid_input(format!("unknown inference route: {}", request.route))
    })?;

    if state.runtime_model_loading_id().lock().await.is_some() {
        return Err(CommandError::invalid_input(
            "cannot change inference route while a local model is loading",
        ));
    }

    if route == InferenceMode::ThirdParty {
        let mut runtime_mgr = state.runtime_manager().lock().await;
        let snap = runtime_mgr.status_snapshot();
        if snap.lifecycle_state == RuntimeLifecycleState::Starting.as_str() && !snap.model_loaded {
            return Err(CommandError::invalid_input(
                "cannot switch to third-party while a local model is loading",
            ));
        }
        if runtime_mgr.is_runtime_active() {
            let _ = runtime_mgr.stop_runtime().await;
        }
    }

    let models: Vec<ModelEntry> = {
        let manager = state.model_manager().lock().await;
        manager.list_models().into_iter().cloned().collect()
    };

    let run_connectivity_test = route == InferenceMode::ThirdParty
        && request
            .selected_model_id
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty());

    let mut inference = state.inference_manager().lock().await;
    let mut config = inference.config().clone();
    config.mode = route;
    config.initialized = true;

    if let Some(id) = request
        .selected_model_id
        .filter(|value| !value.trim().is_empty())
    {
        let valid = models.iter().any(|model| match route {
            InferenceMode::ThirdParty => is_third_party_model(model) && model.id == id,
            InferenceMode::Local => is_local_model(model) && model.id == id,
            InferenceMode::Deterministic => false,
        });
        if valid {
            config.selected_model_id = Some(id);
        }
    } else {
        let selection_matches_route = config
            .selected_model_id
            .as_ref()
            .is_some_and(|id| {
                models.iter().any(|model| match route {
                    InferenceMode::ThirdParty => is_third_party_model(model) && model.id == *id,
                    InferenceMode::Local => is_local_model(model) && model.id == *id,
                    InferenceMode::Deterministic => false,
                })
            });
        if !selection_matches_route {
            config.selected_model_id = None;
            if route == InferenceMode::ThirdParty {
                config.health = Default::default();
            }
        }
    }

    config = reconcile_config(config, &models);

    if run_connectivity_test {
        let mut connectivity_test: Option<(bool, String)> = None;
        if let Some(id) = config.selected_model_id.clone() {
            drop(inference);
            connectivity_test = Some(
                run_third_party_connectivity_test_for_config(state.inner(), &mut config, &id)
                    .await,
            );
            let mut inference = state.inference_manager().lock().await;
            *inference.config_mut() = config.clone();
            inference
                .save()
                .await
                .map_err(|e| CommandError::from(aisec_core::AisecError::internal(e.to_string())))?;
            return Ok(config_to_dto_with_connectivity_test(
                &config,
                &models,
                connectivity_test,
            ));
        }
    }

    *inference.config_mut() = config.clone();
    inference
        .save()
        .await
        .map_err(|e| CommandError::from(aisec_core::AisecError::internal(e.to_string())))?;
    Ok(config_to_dto(&config, &models))
}

#[tauri::command]
pub async fn runtime_test_connectivity(
    state: State<'_, AppState>,
) -> CommandResult<aisec_judge::JudgeConnectivityResult> {
    let inference = state.inference_manager().lock().await;
    let manager = state.model_manager().lock().await;
    let mut runtime_mgr = state.runtime_manager().lock().await;
    let mut session = open_gateway_session(
        state.data_dir(),
        &inference,
        &manager,
        state.model_provider().clone(),
        &mut runtime_mgr,
    )
    .await?;
    let result = session
        .test_connectivity()
        .await
        .map_err(|e| CommandError::from(aisec_core::AisecError::internal(e.to_string())))?;
    Ok(connectivity_to_judge(result))
}

#[tauri::command]
pub async fn runtime_test_inference(
    state: State<'_, AppState>,
) -> CommandResult<aisec_judge::JudgeConnectivityResult> {
    let inference = state.inference_manager().lock().await;
    let manager = state.model_manager().lock().await;
    let mut runtime_mgr = state.runtime_manager().lock().await;
    let mut session = open_gateway_session(
        state.data_dir(),
        &inference,
        &manager,
        state.model_provider().clone(),
        &mut runtime_mgr,
    )
    .await?;
    let result = session
        .test_inference()
        .await
        .map_err(|e| CommandError::from(aisec_core::AisecError::internal(e.to_string())))?;
    Ok(connectivity_to_judge(result))
}

fn map_runtime_err(err: aisec_runtime::RuntimeError) -> CommandError {
    match err {
        aisec_runtime::RuntimeError::Unavailable => {
            CommandError::invalid_input(
                "Embedded libllama engine is unavailable — reinitialize the engine from AI Runtime",
            )
        }
        other => CommandError::from(aisec_core::AisecError::internal(other.to_string())),
    }
}
