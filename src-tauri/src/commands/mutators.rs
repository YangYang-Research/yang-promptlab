use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tauri::State;

use promptlab_storage::{MutatorSettingsRepository, UpdateMutatorSettings};

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MutatorSettingsDto {
    pub enabled_mutators: Vec<String>,
    /// category_id → ordered mutator ids
    pub category_mutators: BTreeMap<String, Vec<String>>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMutatorSettingsRequest {
    pub enabled_mutators: Vec<String>,
    pub category_mutators: BTreeMap<String, Vec<String>>,
}

fn mutator_settings_dto(row: promptlab_storage::MutatorSettings) -> MutatorSettingsDto {
    MutatorSettingsDto {
        enabled_mutators: row.enabled_mutators,
        category_mutators: row.category_mutators,
        updated_at: crate::dto::ts(row.updated_at),
    }
}

#[tauri::command]
pub async fn mutator_settings_get(
    state: State<'_, AppState>,
) -> CommandResult<MutatorSettingsDto> {
    let row = state
        .repositories()
        .mutator_settings()
        .get()
        .await
        .map_err(CommandError::from)?;
    Ok(mutator_settings_dto(row))
}

#[tauri::command]
pub async fn mutator_settings_set(
    state: State<'_, AppState>,
    request: UpdateMutatorSettingsRequest,
) -> CommandResult<MutatorSettingsDto> {
    let row = state
        .repositories()
        .mutator_settings()
        .update(UpdateMutatorSettings {
            enabled_mutators: request.enabled_mutators,
            category_mutators: request.category_mutators,
        })
        .await
        .map_err(CommandError::from)?;
    Ok(mutator_settings_dto(row))
}
