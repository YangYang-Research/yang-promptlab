use aisec_core::{init_logging, LogGuard, LogOptions};
use tauri::{App, Manager};

use crate::error::CommandResult;

/// Resolve log directory under the Tauri app data path and initialize tracing.
pub fn init_app_logging(app: &App) -> CommandResult<LogGuard> {
    let log_dir = app
        .path()
        .app_data_dir()
        .map_err(|err| aisec_core::AisecError::config(err.to_string()))?
        .join("logs");

    init_logging(
        LogOptions::bootstrap("aisec-desktop").with_log_dir(log_dir),
    )
    .map_err(crate::error::CommandError::from)
}
