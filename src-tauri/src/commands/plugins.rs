//! Plugin manager IPC commands.

use promptlab_plugin_host::{PluginManager, PluginState, PluginType};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::plugin_service::save_plugin_state;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRecordDto {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: String,
    pub plugin_type: String,
    pub language: String,
    pub state: String,
    pub enabled: bool,
    pub install_path: String,
    pub hooks: Vec<String>,
}

fn state_label(state: PluginState) -> &'static str {
    match state {
        PluginState::Discovered => "discovered",
        PluginState::Installed => "installed",
        PluginState::Enabled => "enabled",
        PluginState::Loaded => "loaded",
        PluginState::Active => "active",
        PluginState::Disabled => "disabled",
        PluginState::Error => "error",
    }
}

fn record_to_dto(record: &promptlab_plugin_host::PluginRecord) -> PluginRecordDto {
    let mut hooks = Vec::new();
    if let Some(h) = record.hooks.discover.as_deref() {
        hooks.push(h.to_string());
    }
    if let Some(h) = record.hooks.execute_attack.as_deref() {
        hooks.push(h.to_string());
    }
    if let Some(h) = record.hooks.evaluate.as_deref() {
        hooks.push(h.to_string());
    }
    if let Some(h) = record.hooks.render_report.as_deref() {
        hooks.push(h.to_string());
    }

    PluginRecordDto {
        id: record.id.clone(),
        name: record.name.clone(),
        version: record.version.clone(),
        api_version: record.api_version.clone(),
        plugin_type: record.plugin_type.as_str().to_string(),
        language: record.language.as_str().to_string(),
        state: state_label(record.state).to_string(),
        enabled: record.enabled,
        install_path: record.install_path.to_string_lossy().into_owned(),
        hooks,
    }
}

pub async fn plugins_list_op(state: &AppState) -> CommandResult<Vec<PluginRecordDto>> {
    let manager = state.plugin_manager().lock().await;
    Ok(manager.list().into_iter().map(record_to_dto).collect())
}

pub async fn plugins_refresh_op(state: &AppState) -> CommandResult<Vec<PluginRecordDto>> {
    let mut manager = state.plugin_manager().lock().await;
    manager
        .discover()
        .map_err(CommandError::from)?;
    promptlab_plugin_host::restore_enabled(&mut manager, state.config_dir())
        .map_err(CommandError::from)?;
    Ok(manager.list().into_iter().map(record_to_dto).collect())
}

pub async fn plugins_enable_op(state: &AppState, plugin_id: String) -> CommandResult<PluginRecordDto> {
    let mut manager = state.plugin_manager().lock().await;
    manager
        .enable(&plugin_id)
        .map_err(CommandError::from)?;
    save_plugin_state(&manager, state.config_dir()).map_err(CommandError::from)?;
    manager
        .get(&plugin_id)
        .map(record_to_dto)
        .ok_or_else(|| CommandError::invalid_input(format!("plugin not found: {plugin_id}")))
}

pub async fn plugins_disable_op(state: &AppState, plugin_id: String) -> CommandResult<PluginRecordDto> {
    let mut manager = state.plugin_manager().lock().await;
    manager
        .disable(&plugin_id)
        .map_err(CommandError::from)?;
    save_plugin_state(&manager, state.config_dir()).map_err(CommandError::from)?;
    manager
        .get(&plugin_id)
        .map(record_to_dto)
        .ok_or_else(|| CommandError::invalid_input(format!("plugin not found: {plugin_id}")))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginsInfoDto {
    pub plugins_dir: String,
    pub installed_count: usize,
    pub enabled_count: usize,
    pub discovery_count: usize,
    pub attack_count: usize,
    pub judge_count: usize,
}

pub async fn plugins_info_op(state: &AppState) -> CommandResult<PluginsInfoDto> {
    let manager = state.plugin_manager().lock().await;
    let list = manager.list();
    Ok(PluginsInfoDto {
        plugins_dir: state.plugins_dir().to_string_lossy().into_owned(),
        installed_count: list.len(),
        enabled_count: list.iter().filter(|r| r.enabled).count(),
        discovery_count: manager.by_type(PluginType::Discovery).len(),
        attack_count: manager.by_type(PluginType::Attack).len(),
        judge_count: manager.by_type(PluginType::Judge).len(),
    })
}

#[tauri::command]
pub async fn plugins_list(state: State<'_, AppState>) -> CommandResult<Vec<PluginRecordDto>> {
    plugins_list_op(&state).await
}

#[tauri::command]
pub async fn plugins_refresh(state: State<'_, AppState>) -> CommandResult<Vec<PluginRecordDto>> {
    plugins_refresh_op(&state).await
}

#[tauri::command]
pub async fn plugins_enable(
    state: State<'_, AppState>,
    plugin_id: String,
) -> CommandResult<PluginRecordDto> {
    plugins_enable_op(&state, plugin_id).await
}

#[tauri::command]
pub async fn plugins_disable(
    state: State<'_, AppState>,
    plugin_id: String,
) -> CommandResult<PluginRecordDto> {
    plugins_disable_op(&state, plugin_id).await
}

#[tauri::command]
pub async fn plugins_info(state: State<'_, AppState>) -> CommandResult<PluginsInfoDto> {
    plugins_info_op(&state).await
}
