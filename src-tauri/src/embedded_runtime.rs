//! Load persisted runtime configuration at app startup (no model preload).

use std::path::Path;

use aisec_core::AisecResult;
use aisec_inference::config::InferenceMode;
use aisec_models::ModelProvider;
use aisec_runtime::RuntimeManager;
use tauri::{AppHandle, Manager};
use tracing::{info, warn};

use crate::commands::runtime::{
    load_model_with_loading_cache, prime_loading_configuration_cache, prime_runtime_configuration_cache,
    set_runtime_model_loading,
};
use crate::runtime_watch;
use crate::state::AppState;

/// Load runtime configuration from disk at app startup. Does not install or start.
pub async fn bootstrap_runtime_manager(
    _app: &AppHandle,
    data_dir: &Path,
) -> AisecResult<(RuntimeManager, bool)> {
    let mut manager = RuntimeManager::new(data_dir);

    match manager.bootstrap().await {
        Ok(()) => {
            info!(
                state = manager.lifecycle_state().as_str(),
                "embedded libllama runtime configuration loaded"
            );
            Ok((manager, false))
        }
        Err(err) => {
            warn!(error = %err, "embedded libllama runtime bootstrap failed");
            Ok((manager, false))
        }
    }
}

/// Detect hardware synchronously during app startup (before the window is shown).
pub async fn detect_hardware_on_startup(manager: &mut RuntimeManager) {
    match manager.refresh_hardware().await {
        Ok(profile) => {
            info!(
                cpu_cores = profile.cpu_cores,
                ram_bytes = profile.ram_bytes,
                "hardware profile detected on startup"
            );
        }
        Err(err) => warn!(error = %err, "hardware detect on startup failed"),
    }
}

/// When AI inference is configured for local mode, start the runtime and reload the last model.
pub async fn resume_local_runtime_on_startup(app: &AppHandle, state: &AppState) {
    let settings = {
        let inference = state.inference_manager().lock().await;
        inference.config().clone()
    };

    if !settings.initialized || settings.mode != InferenceMode::Local {
        return;
    }

    let model_id = match settings.selected_model_id.clone() {
        Some(id) => id,
        None => {
            info!("local runtime auto-resume skipped: no selected model");
            return;
        }
    };

    let file_path = {
        let manager = state.model_manager().lock().await;
        let Some(entry) = manager.get_model(&model_id) else {
            warn!(model_id = %model_id, "local runtime auto-resume skipped: model not found");
            return;
        };
        if entry.provider == ModelProvider::Remote {
            return;
        }
        if !entry.file_path.exists() {
            warn!(
                model_id = %model_id,
                path = %entry.file_path.display(),
                "local runtime auto-resume skipped: model file missing"
            );
            return;
        }
        entry.file_path.clone()
    };

    set_runtime_model_loading(state, Some(model_id.clone())).await;
    prime_loading_configuration_cache(state, &model_id).await;

    let mut runtime = state.runtime_manager().lock().await;
    if !runtime.supervisor().runtime_available() {
        set_runtime_model_loading(state, None).await;
        info!("local runtime auto-resume skipped: embedded libllama unavailable");
        return;
    }

    if runtime.is_model_loaded_at(&file_path).await {
        runtime.sync_lifecycle_from_supervisor();
        set_runtime_model_loading(state, None).await;
        prime_runtime_configuration_cache(state, &runtime).await;
        info!(model_id = %model_id, "local runtime auto-resume: model already loaded");
        runtime_watch::spawn_runtime_watch(app.clone());
        return;
    }

    if let Err(err) = runtime.start_runtime().await {
        warn!(error = %err, "local runtime auto-resume: failed to start runtime");
        set_runtime_model_loading(state, None).await;
        return;
    }

    match load_model_with_loading_cache(state, &mut runtime, &file_path, &model_id).await {
        Ok(()) => {
            info!(model_id = %model_id, "local runtime auto-resume: model loaded");
            runtime_watch::spawn_runtime_watch(app.clone());
        }
        Err(err) => {
            warn!(
                error = %err,
                model_id = %model_id,
                "local runtime auto-resume: failed to load model"
            );
        }
    }
}
