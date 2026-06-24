//! Load persisted runtime configuration at app startup (no install/start).

use std::path::{Path, PathBuf};

use aisec_core::AisecResult;
use aisec_models::ModelProvider;
use aisec_runtime::RuntimeManager;
use tauri::{AppHandle, Manager};
use tracing::{info, warn};

use crate::ai_inference_settings::{load_settings, AiInferenceRoute};
use crate::commands::runtime::{
    load_model_with_loading_cache, prime_loading_configuration_cache, prime_runtime_configuration_cache,
    set_runtime_model_loading,
};
use crate::runtime_watch;
use crate::state::AppState;

fn bundled_resource_binary(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .resource_dir()
        .ok()
        .map(|dir| aisec_runtime::bundled_llama_server_binary(&dir))
        .filter(|path| path.is_file())
}

/// Load runtime configuration from disk at app startup. Does not install or start.
pub async fn bootstrap_runtime_manager(
    app: &AppHandle,
    data_dir: &Path,
) -> AisecResult<(RuntimeManager, bool)> {
    let bundled = bundled_resource_binary(app);
    let mut manager = RuntimeManager::new(data_dir, bundled);

    match manager.bootstrap().await {
        Ok(()) => {
            info!(
                state = manager.lifecycle_state().as_str(),
                "AI runtime configuration loaded"
            );
            Ok((manager, false))
        }
        Err(err) => {
            warn!(error = %err, "AI runtime bootstrap failed");
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
    let data_dir = state.data_dir();
    let settings = match load_settings(data_dir).await {
        Ok(settings) => settings,
        Err(err) => {
            warn!(error = %err, "local runtime auto-resume skipped: settings unavailable");
            return;
        }
    };

    if !settings.initialized || settings.route != AiInferenceRoute::Local {
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
    if !runtime.supervisor().binary_available() {
        set_runtime_model_loading(state, None).await;
        info!("local runtime auto-resume skipped: llama-server binary missing");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_path_helper_compiles() {
        let _ = bundled_resource_binary as fn(&AppHandle) -> Option<PathBuf>;
    }
}
