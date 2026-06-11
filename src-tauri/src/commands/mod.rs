use serde::Serialize;

use crate::error::CommandResult;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Serialize)]
pub struct AppInfoResponse {
    pub name: &'static str,
    pub version: &'static str,
    pub identifier: &'static str,
}

/// Bootstrap health check command for IPC wiring verification.
#[tauri::command]
pub fn health() -> CommandResult<HealthResponse> {
    Ok(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// Returns static application metadata.
#[tauri::command]
pub fn app_info() -> CommandResult<AppInfoResponse> {
    Ok(AppInfoResponse {
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        identifier: "com.aisec.desktop",
    })
}
