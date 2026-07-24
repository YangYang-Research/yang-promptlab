use promptlab_core::{init_logging, EnvironmentPaths, LogGuard, LogOptions};
use tauri::App;

use crate::error::CommandResult;

/// Initialize tracing (console) under PromptLab logs directory.
pub fn init_app_logging(environment: &EnvironmentPaths) -> CommandResult<LogGuard> {
    init_logging(
        LogOptions::bootstrap("promptlab-desktop")
            .with_log_dir(environment.logs.clone())
            .with_default_filter(
                "info,promptlab_desktop=debug,promptlab_core=debug,promptlab_desktop=debug,promptlab_core=debug",
            ),
    )
    .map_err(crate::error::CommandError::from)
}
