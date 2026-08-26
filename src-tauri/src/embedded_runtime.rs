//! Load persisted runtime configuration at app startup (no model preload).

use std::path::Path;

use promptlab_core::PromptLabResult;
use promptlab_runtime::RuntimeManager;
use promptlab_storage::Database;
use tauri::AppHandle;
use tracing::{info, warn};

/// Load runtime configuration from disk at app startup. Does not install or start.
pub async fn bootstrap_runtime_manager(
    _app: &AppHandle,
    data_dir: &Path,
    db: &Database,
) -> PromptLabResult<(RuntimeManager, bool)> {
    let mut manager = RuntimeManager::new(data_dir, Some(db.clone()));

    match manager.bootstrap().await {
        Ok(()) => {
            info!(
                state = manager.lifecycle_state().as_str(),
                "AI runtime configuration loaded (remote-only)"
            );
            Ok((manager, false))
        }
        Err(err) => {
            warn!(error = %err, "AI runtime bootstrap failed");
            Ok((manager, false))
        }
    }
}

/// Load persisted hardware profile at startup (detect only when missing).
pub async fn detect_hardware_on_startup(manager: &mut RuntimeManager) {
    match manager.ensure_hardware_profile().await {
        Ok(profile) => {
            info!(
                cpu_cores = profile.cpu_cores,
                ram_bytes = profile.ram_bytes,
                "hardware profile loaded on startup"
            );
        }
        Err(err) => warn!(error = %err, "hardware profile load on startup failed"),
    }
}
