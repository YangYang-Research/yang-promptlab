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
    let data_dir = state.data_dir().to_path_buf();
    let db_path = db::resolve_db_path(&data_dir);

    {
        let mut runtime = state.runtime_manager().lock().await;
        let _ = runtime.stop_runtime().await;
    }

    state.database().close().await;

    if data_dir.exists() {
        std::fs::remove_dir_all(&data_dir).map_err(|err| {
            CommandError::from(aisec_core::AisecError::internal(format!(
                "failed to remove application data directory: {err}"
            )))
        })?;
        info!(path = %data_dir.display(), "application data directory removed");
    }

    let default_db = data_dir.join(db::DB_FILENAME);
    if db_path != default_db {
        if db_path.is_file() {
            let _ = std::fs::remove_file(&db_path);
            remove_sqlite_sidecars(&db_path);
        } else if db_path.is_dir() {
            let _ = std::fs::remove_dir_all(&db_path);
        }
    }

    info!("clear all data complete; relaunching application");
    app.restart();
    Ok(())
}

#[tauri::command]
pub async fn app_clear_all_data(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    app_clear_all_data_op(state.inner(), &app).await
}
