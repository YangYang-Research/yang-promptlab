//! Application-level commands (data reset, relaunch).

use std::path::PathBuf;

use tauri::{AppHandle, State};
use tracing::info;

use crate::db;
use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

fn remove_sqlite_sidecars(db_path: &std::path::Path) {
    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{suffix}", db_path.display()));
        let _ = std::fs::remove_file(sidecar);
    }
}

pub async fn app_clear_all_data_op(state: &AppState, app: &AppHandle) -> CommandResult<()> {
    let root = state.root_dir().to_path_buf();
    let db_path = db::resolve_db_path(state.workspaces_dir());

    {
        let mut runtime = state.runtime_manager().lock().await;
        let _ = runtime.stop_runtime().await;
    }

    state.database().close().await;

    if root.exists() {
        std::fs::remove_dir_all(&root).map_err(|err| {
            CommandError::from(promptlab_core::PromptLabError::internal(format!(
                "failed to remove PromptLab root directory: {err}"
            )))
        })?;
        info!(path = %root.display(), "PromptLab root directory removed");
    }

    if db_path != state.environment().database_path() {
        if db_path.is_file() {
            let _ = std::fs::remove_file(&db_path);
            remove_sqlite_sidecars(&db_path);
        } else if db_path.is_dir() {
            let _ = std::fs::remove_dir_all(&db_path);
        }
    }

    state.event_bus().info(
        promptlab_core::LogCategory::Application,
        "Application Data Cleared",
        "app",
        "clear_all_data",
        "All PromptLab data removed; relaunching",
    );
    info!("clear all data complete; relaunching application");
    app.restart()
}

#[tauri::command]
pub async fn app_clear_all_data(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    app_clear_all_data_op(state.inner(), &app).await
}
