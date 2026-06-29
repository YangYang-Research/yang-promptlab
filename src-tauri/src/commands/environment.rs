//! Environment and diagnostics IPC commands.

use std::path::PathBuf;

use aisec_core::{
    ensure_environment, list_log_files, load_environment_config, read_log_tail, resolve_paths,
    save_environment_config, EnvironmentConfig, EnvironmentPaths, LogCategory, OcsfEvent,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentStatusDto {
    pub root: String,
    pub config: String,
    pub workspaces: String,
    pub models: String,
    pub runtime: String,
    pub logs: String,
    pub plugins: String,
    pub cache: String,
    pub temp: String,
    pub backups: String,
    pub database: String,
    pub writable: bool,
    pub message: String,
}

impl From<&EnvironmentPaths> for EnvironmentStatusDto {
    fn from(paths: &EnvironmentPaths) -> Self {
        Self {
            root: paths.root.display().to_string(),
            config: paths.config.display().to_string(),
            workspaces: paths.workspaces.display().to_string(),
            models: paths.models.display().to_string(),
            runtime: paths.runtime.display().to_string(),
            logs: paths.logs.display().to_string(),
            plugins: paths.plugins.display().to_string(),
            cache: paths.cache.display().to_string(),
            temp: paths.temp.display().to_string(),
            backups: paths.backups.display().to_string(),
            database: paths.database_path().display().to_string(),
            writable: true,
            message: "Environment validated".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentUpdateRequest {
    pub root: Option<String>,
    pub workspaces: Option<String>,
    pub models: Option<String>,
    pub runtime: Option<String>,
    pub logs: Option<String>,
    pub plugins: Option<String>,
    pub cache: Option<String>,
    pub temp: Option<String>,
    pub backups: Option<String>,
}

fn parse_optional_path(value: Option<String>) -> Option<PathBuf> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

#[tauri::command]
pub async fn environment_get(state: State<'_, AppState>) -> CommandResult<EnvironmentStatusDto> {
    Ok(EnvironmentStatusDto::from(state.environment()))
}

#[tauri::command]
pub async fn environment_open_root(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    use tauri_plugin_opener::OpenerExt;

    let path = state.environment().root.clone();
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|err| {
            CommandError::from(aisec_core::AisecError::internal(format!(
                "failed to open root directory: {err}"
            )))
        })?;
    Ok(())
}

#[tauri::command]
pub async fn environment_update(
    state: State<'_, AppState>,
    request: EnvironmentUpdateRequest,
) -> CommandResult<EnvironmentStatusDto> {
    let mut config = load_environment_config(&state.environment().root);
    if let Some(root) = parse_optional_path(request.root) {
        config.root = Some(root);
    }
    config.workspaces = parse_optional_path(request.workspaces).or(config.workspaces);
    config.models = parse_optional_path(request.models).or(config.models);
    config.runtime = parse_optional_path(request.runtime).or(config.runtime);
    config.logs = parse_optional_path(request.logs).or(config.logs);
    config.plugins = parse_optional_path(request.plugins).or(config.plugins);
    config.cache = parse_optional_path(request.cache).or(config.cache);
    config.temp = parse_optional_path(request.temp).or(config.temp);
    config.backups = parse_optional_path(request.backups).or(config.backups);

    let paths = resolve_paths(&config);
    ensure_environment(&paths).map_err(CommandError::from)?;
    save_environment_config(&paths, &config).map_err(CommandError::from)?;
    state.event_bus().info(
        LogCategory::Settings,
        "Environment Updated",
        "environment",
        "ipc",
        "Environment paths updated",
    );
    Ok(EnvironmentStatusDto::from(&paths))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFileInfoDto {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsTailResponse {
    pub path: String,
    pub content: String,
}

#[tauri::command]
pub async fn logs_list_files(state: State<'_, AppState>) -> CommandResult<Vec<LogFileInfoDto>> {
    let files = list_log_files(&state.logs_dir());
    Ok(files
        .into_iter()
        .map(|path| LogFileInfoDto {
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path: path.display().to_string(),
        })
        .collect())
}

#[tauri::command]
pub async fn logs_tail(
    state: State<'_, AppState>,
    file_name: String,
    max_bytes: Option<usize>,
) -> CommandResult<LogsTailResponse> {
    let path = state.logs_dir().join(file_name.trim());
    if !path.starts_with(state.logs_dir()) {
        return Err(CommandError::invalid_input("invalid log file path"));
    }
    let content = read_log_tail(&path, max_bytes.unwrap_or(64 * 1024))
        .map_err(|e| CommandError::from(aisec_core::AisecError::internal(e.to_string())))?;
    Ok(LogsTailResponse {
        path: path.display().to_string(),
        content,
    })
}

#[tauri::command]
pub async fn logs_recent_events(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> CommandResult<Vec<OcsfEvent>> {
    Ok(state.event_ring().recent(limit.unwrap_or(200)))
}

#[tauri::command]
pub async fn logs_open_folder(state: State<'_, AppState>) -> CommandResult<String> {
    Ok(state.logs_dir().display().to_string())
}
