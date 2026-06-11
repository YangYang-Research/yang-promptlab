use aisec_core::LogGuard;

/// Shared application state managed by Tauri.
pub struct AppState {
    pub _log_guard: LogGuard,
}

impl AppState {
    pub fn new(log_guard: LogGuard) -> Self {
        Self {
            _log_guard: log_guard,
        }
    }
}
