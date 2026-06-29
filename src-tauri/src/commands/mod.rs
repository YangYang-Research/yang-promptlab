use serde::Serialize;
use tauri::State;

use aisec_storage::ProjectRepository;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

pub mod app;
pub mod attack;
pub mod auth;
pub mod discovery;
pub mod domain;
pub mod environment;
pub mod generator;
pub mod models;
pub mod planner;
pub mod plugins;
pub mod projects;
pub mod runtime;
pub mod scan;
pub mod security;
pub mod target_profile;
pub mod wizard_scan;

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
        name: "PromptLab",
        version: env!("CARGO_PKG_VERSION"),
        identifier: "com.promptlab.desktop",
    })
}

/// Database connectivity check — proves the database is reachable from a command
/// via the shared [`AppState`]/repository manager.
#[derive(Debug, Serialize)]
pub struct DbHealthResponse {
    pub connected: bool,
    pub project_count: usize,
}

#[tauri::command]
pub async fn db_health(state: State<'_, AppState>) -> CommandResult<DbHealthResponse> {
    // Exercise the repository manager against the live pool.
    let projects = state
        .repositories()
        .projects()
        .list()
        .await
        .map_err(CommandError::from)?;

    Ok(DbHealthResponse {
        connected: !state.database().is_closed(),
        project_count: projects.len(),
    })
}
