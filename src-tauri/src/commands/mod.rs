use serde::Serialize;
use tauri::State;

use promptlab_storage::ProjectRepository;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

pub mod app;
pub mod agent_memory;
pub mod attack;
pub mod attack_catalog;
pub mod auth;
pub mod domain;
pub mod environment;
pub mod generator;
pub mod models;
pub mod mutators;
pub mod planner;
pub mod plugins;
pub mod projects;
pub mod proxy;
pub mod runtime;
pub mod scan;
pub mod scan_execution;
pub mod project_summary;
pub mod scan_recommendations;
pub mod security;
pub mod target_profile;
pub mod yazg;
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
/// via the shared [`AppState`] repository manager.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbHealthResponse {
    pub connected: bool,
    pub path: String,
    pub size_bytes: u64,
}

#[tauri::command]
pub async fn db_health(state: State<'_, AppState>) -> CommandResult<DbHealthResponse> {
    let path = state.environment().database_path();
    let size_bytes = std::fs::metadata(&path)
        .map(|meta| meta.len())
        .unwrap_or(0);

    // Exercise the repository manager against the live pool.
    let _projects = state
        .repositories()
        .projects()
        .list()
        .await
        .map_err(CommandError::from)?;

    Ok(DbHealthResponse {
        connected: !state.database().is_closed(),
        path: path.display().to_string(),
        size_bytes,
    })
}
