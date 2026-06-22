//! Load persisted runtime configuration at app startup (no install/start).

use std::path::{Path, PathBuf};

use aisec_core::AisecResult;
use aisec_runtime::RuntimeManager;
use tauri::{AppHandle, Manager};
use tracing::info;

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
            tracing::warn!(error = %err, "AI runtime bootstrap failed");
            Ok((manager, false))
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
