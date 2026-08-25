//! Embedded AI runtime IPC — lifecycle, health, benchmark, logs, hardware.

use promptlab_models::ModelEntry;
use promptlab_runtime::{
    hardware::HardwareDetector, RuntimeBenchmarkResult, RuntimeHardwareProfile,
    RuntimeHealthReport, RuntimeLogEntry, RuntimeStatusSnapshot,
};
use serde::Deserialize;
use serde::Serialize;
use tauri::{AppHandle, State};
use time::OffsetDateTime;

use promptlab_inference::config::{AiRuntimeConfiguration, InferenceMode};
use crate::inference_settings::{
    apply_third_party_health_check, config_to_dto, config_to_dto_with_connectivity_test,
    format_health_check_timestamp, is_third_party_model, parse_route, reconcile_config,
    third_party_status_label, AiInferenceSettingsDto,
};
use crate::inference_host::{connectivity_to_judge, open_gateway_session};
use crate::commands::models::test_third_party_model_connection;
use crate::error::{CommandError, CommandResult};
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
    pub disk_free_bytes: Option<u64>,
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
            disk_free_bytes: value.disk_free_bytes,
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
    pub model_test_in_progress: bool,
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

async fn status_dto_for_manager(manager: &promptlab_runtime::RuntimeManager) -> RuntimeStatusDto {
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
    _app: AppHandle,
    _state: State<'_, AppState>,
) -> CommandResult<RuntimeStatusDto> {
    Err(CommandError::invalid_input(
        "embedded llama.cpp runtime install has been removed — configure a remote AI provider",
    ))
}

#[tauri::command]
pub async fn runtime_repair(
    _app: AppHandle,
    _state: State<'_, AppState>,
) -> CommandResult<RuntimeStatusDto> {
    Err(CommandError::invalid_input(
        "embedded llama.cpp runtime repair has been removed — configure a remote AI provider",
    ))
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
    _app: AppHandle,
    _state: State<'_, AppState>,
    _request: RuntimeLoadModelRequest,
) -> CommandResult<RuntimeConfigurationDto> {
    Err(CommandError::invalid_input(
        "embedded GGUF load has been removed — configure a remote AI provider or Ollama over HTTP",
    ))
}

#[tauri::command]
pub async fn runtime_unload_model(
    _state: State<'_, AppState>,
) -> CommandResult<RuntimeConfigurationDto> {
    Err(CommandError::invalid_input(
        "embedded GGUF unload has been removed — no in-process model is loaded",
    ))
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
pub async fn runtime_traffic_stats(
    state: State<'_, AppState>,
    window_ms: Option<u64>,
    bucket_ms: Option<u64>,
) -> CommandResult<promptlab_inference::TrafficSnapshot> {
    crate::traffic_persist::traffic_snapshot_from_db(
        &state.repositories(),
        window_ms.unwrap_or(60_000),
        bucket_ms.unwrap_or(1_000),
    )
    .await
    .map_err(CommandError::from)
}

#[tauri::command]
pub async fn runtime_token_usage(
    state: State<'_, AppState>,
) -> CommandResult<promptlab_inference::TokenUsageSnapshot> {
    Ok(crate::token_usage_persist::usage_snapshot(state.database()).await)
}

#[tauri::command]
pub async fn runtime_token_usage_reset(
    state: State<'_, AppState>,
) -> CommandResult<promptlab_inference::TokenUsageSnapshot> {
    crate::token_usage_persist::reset_usage(state.database())
        .await
        .map_err(CommandError::invalid_input)
}

#[tauri::command]
pub async fn runtime_benchmark(_state: State<'_, AppState>) -> CommandResult<RuntimeBenchmarkResult> {
    Err(CommandError::invalid_input(
        "local runtime benchmark has been removed — embedded llama.cpp is unavailable",
    ))
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

    let profile = HardwareDetector::with_db(state.data_dir(), state.database().clone())
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
    let profile = HardwareDetector::with_db(state.data_dir(), state.database().clone())
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

/// Re-check AI Runtime connectivity when the desktop app starts.
pub(crate) async fn startup_connectivity_check(state: &AppState) {
    let models: Vec<ModelEntry> = {
        let manager = state.model_manager().lock().await;
        manager.list_models().into_iter().cloned().collect()
    };

    let mut config = {
        let mut inference = state.inference_manager().lock().await;
        let _ = inference.load().await;
        inference.config().clone()
    };

    if config.selected_model_id.is_none() {
        return;
    }

    config = reconcile_config(config, &models);

    match config.mode {
        InferenceMode::ThirdParty => {
            let Some(model_id) = config.selected_model_id.clone() else {
                tracing::info!("startup connectivity check skipped: no selected third-party model");
                return;
            };
            let (ok, detail) =
                run_third_party_connectivity_test_for_config(state, &mut config, &model_id).await;
            {
                let mut inference = state.inference_manager().lock().await;
                *inference.config_mut() = config;
                if let Err(err) = inference.save().await {
                    tracing::warn!(error = %err, "failed to persist startup connectivity result");
                }
            }
            if ok {
                tracing::info!(model_id = %model_id, "startup third-party connectivity check ok");
            } else {
                tracing::warn!(
                    model_id = %model_id,
                    detail = %detail,
                    "startup third-party connectivity check failed"
                );
            }
            if let Err(err) = runtime_configuration_for_state(state).await {
                tracing::warn!(error = %err, "failed to refresh runtime cache after startup check");
            }
        }
        InferenceMode::Local => {
            let mut runtime = state.runtime_manager().lock().await;
            match runtime.run_health_check().await {
                Ok(report) => {
                    tracing::info!(
                        reachable = report.endpoint_reachable,
                        model_loaded = report.model_loaded,
                        "startup local runtime health check completed"
                    );
                    prime_runtime_configuration_cache(state, &runtime).await;
                }
                Err(err) => {
                    tracing::warn!(error = %err, "startup local runtime health check failed");
                }
            }
        }
        InferenceMode::Deterministic => {}
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
            .map_err(|e| CommandError::from(promptlab_core::PromptLabError::internal(e.to_string())))?;
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

async fn store_runtime_configuration_cache(state: &AppState, dto: &RuntimeConfigurationDto) {
    *state.runtime_config_cache().lock().await = Some(dto.clone());
}

async fn assemble_runtime_configuration(
    models: &[ModelEntry],
    config: &AiRuntimeConfiguration,
    inference: &AiInferenceSettingsDto,
    runtime_manager: &promptlab_runtime::RuntimeManager,
) -> RuntimeConfigurationDto {
    let runtime_status = status_dto_for_manager(runtime_manager).await;

    let selected_model = config.selected_model_id.as_ref().and_then(|id| {
        models.iter().find(|m| &m.id == id)
    });

    // Remote-only product — DTO mode is always third_party.
    RuntimeConfigurationDto {
        mode: "third_party".to_string(),
        status_label: third_party_status_label(
            config,
            inference.third_party_available,
            selected_model,
        ),
        provider: selected_model.map(|m| m.display_provider()),
        model_name: inference.selected_model_name.clone(),
        runtime_name: None,
        runtime_version: None,
        connectivity: if config.health.message.is_empty() {
            None
        } else {
            Some(config.health.message.clone())
        },
        last_health_check: config.health.checked_at.clone(),
        model_load_in_progress: false,
        model_test_in_progress: false,
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
        message: "Busy".into(),
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

    RuntimeConfigurationDto {
        mode: "third_party".to_string(),
        status_label: third_party_status_label(
            config,
            inference.third_party_available,
            selected_model,
        ),
        provider: selected_model.map(|m| m.display_provider()),
        model_name: inference.selected_model_name.clone(),
        runtime_name: None,
        runtime_version: None,
        connectivity: if config.health.message.is_empty() {
            None
        } else {
            Some(config.health.message.clone())
        },
        last_health_check: config.health.checked_at.clone(),
        model_load_in_progress: false,
        model_test_in_progress: false,
        settings: inference.clone(),
        runtime_status: fallback_runtime_status_when_busy(),
    }
}

async fn runtime_model_loading_id(state: &AppState) -> Option<String> {
    state.runtime_model_loading_id().lock().await.clone()
}

pub(crate) async fn set_runtime_model_loading(state: &AppState, model_id: Option<String>) {
    *state.runtime_model_loading_id().lock().await = model_id;
}

async fn runtime_model_testing_id(state: &AppState) -> Option<String> {
    state.runtime_model_testing_id().lock().await.clone()
}

pub(crate) async fn set_runtime_model_testing(state: &AppState, model_id: Option<String>) {
    *state.runtime_model_testing_id().lock().await = model_id;
}

fn apply_model_loading_overlay(dto: &mut RuntimeConfigurationDto, loading_model_id: Option<&str>) {
    let Some(model_id) = loading_model_id else {
        return;
    };
    let runtime_name = dto.runtime_name.clone();
    let runtime_version = dto.runtime_version.clone();
    let recommended_runtime = dto.runtime_status.recommended_runtime.clone();
    dto.model_load_in_progress = true;
    dto.status_label = "Loading model".into();
    dto.runtime_status.lifecycle_state = "starting".into();
    dto.runtime_status.model_loaded = false;
    dto.runtime_status.loaded_model_path = None;
    dto.runtime_status.message = "Verifying remote model connectivity…".into();
    dto.runtime_name = runtime_name;
    dto.runtime_version = runtime_version;
    dto.runtime_status.recommended_runtime = recommended_runtime;
    dto.settings.selected_model_id = Some(model_id.to_string());
}

fn apply_model_testing_overlay(dto: &mut RuntimeConfigurationDto, testing_model_id: Option<&str>) {
    let Some(model_id) = testing_model_id else {
        return;
    };
    dto.model_test_in_progress = true;
    dto.status_label = "Verifying model".into();
    dto.settings.selected_model_id = Some(model_id.to_string());
}

/// Refresh the configuration cache while the runtime manager lock is already held.
pub(crate) async fn prime_runtime_configuration_cache(
    state: &AppState,
    runtime_manager: &promptlab_runtime::RuntimeManager,
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

/// Wait until an in-flight model load for `model_id` completes.
pub(crate) async fn wait_for_runtime_model_load(
    state: &AppState,
    model_id: &str,
    file_path: &std::path::Path,
) -> Result<(), promptlab_runtime::RuntimeError> {
    const MAX_WAIT: std::time::Duration = std::time::Duration::from_secs(30 * 60);
    let started = std::time::Instant::now();

    loop {
        let loading = state.runtime_model_loading_id().lock().await.clone();
        if loading.as_deref() != Some(model_id) {
            let manager = state.runtime_manager().lock().await;
            if manager.is_same_model_loaded_at(file_path).await {
                return Ok(());
            }
            if loading.is_some() {
                return Err(promptlab_runtime::RuntimeError::NativeRuntimeError(
                    "another model is loading into the runtime".into(),
                ));
            }
            return Err(promptlab_runtime::RuntimeError::NativeRuntimeError(
                "model load did not complete".into(),
            ));
        }
        if started.elapsed() >= MAX_WAIT {
            return Err(promptlab_runtime::RuntimeError::NativeRuntimeError(
                "timed out waiting for model load".into(),
            ));
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

/// Shared model-load path for manual IPC and startup auto-resume.
pub(crate) async fn load_model_with_loading_cache(
    state: &AppState,
    manager: &mut promptlab_runtime::RuntimeManager,
    file_path: &std::path::Path,
    model_id: &str,
) -> Result<(), promptlab_runtime::RuntimeError> {
    if manager.is_same_model_loaded_at(file_path).await {
        prime_runtime_configuration_cache(state, manager).await;
        return Ok(());
    }

    set_runtime_model_loading(state, Some(model_id.to_string())).await;

    let result = async {
        if !manager.is_same_model_loaded_at(file_path).await {
            manager.on_model_load_started();
            prime_runtime_configuration_cache(state, manager).await;
            manager.load_model_at_path(file_path).await
        } else {
            Ok(())
        }
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
    let loading_model_id = runtime_model_loading_id(state).await;
    let testing_model_id = runtime_model_testing_id(state).await;

    let models: Vec<ModelEntry> = if let Ok(manager) = state.model_manager().try_lock() {
        manager.list_models().into_iter().cloned().collect()
    } else if let Some(cached) = state.runtime_config_cache().lock().await.clone() {
        let mut response = cached;
        apply_model_loading_overlay(&mut response, loading_model_id.as_deref());
        apply_model_testing_overlay(&mut response, testing_model_id.as_deref());
        return Ok(response);
    } else {
        Vec::new()
    };

    let (config, _) = reconcile_inference_config(state, &models).await?;
    let inference = config_to_dto(&config, &models);

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
    apply_model_testing_overlay(&mut response, testing_model_id.as_deref());
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

    if route != InferenceMode::ThirdParty {
        return Err(CommandError::invalid_input(
            "only remote / third-party inference is supported — embedded llama.cpp has been removed",
        ));
    }

    let models: Vec<ModelEntry> = {
        let manager = state.model_manager().lock().await;
        manager.list_models().into_iter().cloned().collect()
    };

    let run_connectivity_test = request
        .selected_model_id
        .as_ref()
        .is_some_and(|value| !value.trim().is_empty());

    let mut inference = state.inference_manager().lock().await;
    let mut config = inference.config().clone();
    config.mode = InferenceMode::ThirdParty;
    config.initialized = true;

    if let Some(id) = request
        .selected_model_id
        .filter(|value| !value.trim().is_empty())
    {
        let valid = models
            .iter()
            .any(|model| is_third_party_model(model) && model.id == id);
        if valid {
            config.selected_model_id = Some(id);
        }
    } else {
        let selection_matches_route = config.selected_model_id.as_ref().is_some_and(|id| {
            models
                .iter()
                .any(|model| is_third_party_model(model) && model.id == *id)
        });
        if !selection_matches_route {
            config.selected_model_id = None;
            config.health = Default::default();
        }
    }

    config = reconcile_config(config, &models);

    if run_connectivity_test {
        if let Some(id) = config.selected_model_id.clone() {
            drop(inference);
            let connectivity_test = Some(
                run_third_party_connectivity_test_for_config(state.inner(), &mut config, &id)
                    .await,
            );
            let mut inference = state.inference_manager().lock().await;
            *inference.config_mut() = config.clone();
            inference
                .save()
                .await
                .map_err(|e| CommandError::from(promptlab_core::PromptLabError::internal(e.to_string())))?;
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
        .map_err(|e| CommandError::from(promptlab_core::PromptLabError::internal(e.to_string())))?;
    Ok(config_to_dto(&config, &models))
}

#[tauri::command]
pub async fn runtime_test_connectivity(
    state: State<'_, AppState>,
) -> CommandResult<promptlab_judge::JudgeConnectivityResult> {
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
        .map_err(|e| CommandError::from(promptlab_core::PromptLabError::internal(e.to_string())))?;
    Ok(connectivity_to_judge(result))
}

#[tauri::command]
pub async fn runtime_test_inference(
    state: State<'_, AppState>,
) -> CommandResult<promptlab_judge::JudgeConnectivityResult> {
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
        .map_err(|e| CommandError::from(promptlab_core::PromptLabError::internal(e.to_string())))?;
    Ok(connectivity_to_judge(result))
}

fn map_runtime_err(err: promptlab_runtime::RuntimeError) -> CommandError {
    match err {
        promptlab_runtime::RuntimeError::Unavailable => {
            CommandError::invalid_input(
                "Embedded libllama engine is unavailable — reinitialize the engine from AI Runtime",
            )
        }
        other => CommandError::from(promptlab_core::PromptLabError::internal(other.to_string())),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgeRoleWeightsDto {
    pub judge: f64,
    pub classifier: f64,
    pub attacker: f64,
    pub default_llm: f64,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateJudgeRoleWeightsRequest {
    pub judge: f64,
    pub classifier: f64,
    pub attacker: f64,
    pub default_llm: f64,
}

fn judge_weights_dto(row: promptlab_storage::JudgeRoleWeights) -> JudgeRoleWeightsDto {
    JudgeRoleWeightsDto {
        judge: row.judge,
        classifier: row.classifier,
        attacker: row.attacker,
        default_llm: row.default_llm,
        updated_at: crate::dto::ts(row.updated_at),
    }
}

#[tauri::command]
pub async fn runtime_judge_role_weights(
    state: State<'_, AppState>,
) -> CommandResult<JudgeRoleWeightsDto> {
    use promptlab_storage::JudgeRoleWeightsRepository;

    let row = state
        .repositories()
        .judge_role_weights()
        .get()
        .await
        .map_err(CommandError::from)?;
    Ok(judge_weights_dto(row))
}

#[tauri::command]
pub async fn runtime_set_judge_role_weights(
    state: State<'_, AppState>,
    request: UpdateJudgeRoleWeightsRequest,
) -> CommandResult<JudgeRoleWeightsDto> {
    use promptlab_storage::{JudgeRoleWeightsRepository, UpdateJudgeRoleWeights};

    let row = state
        .repositories()
        .judge_role_weights()
        .update(UpdateJudgeRoleWeights {
            judge: request.judge,
            classifier: request.classifier,
            attacker: request.attacker,
            default_llm: request.default_llm,
        })
        .await
        .map_err(CommandError::from)?;
    Ok(judge_weights_dto(row))
}
