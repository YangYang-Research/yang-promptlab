//! Dashboard Recent Activity (runtime / model events) persisted in SQLite.

use promptlab_storage::JsonDocumentRepository;
use serde::{Deserialize, Serialize};
use tauri::State;
use time::OffsetDateTime;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

const MAX_ITEMS: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityItemDto {
    pub id: String,
    #[serde(rename = "type")]
    pub activity_type: String,
    pub message: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityRecordRequest {
    #[serde(rename = "type")]
    pub activity_type: String,
    pub message: String,
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct StoredRecentActivity {
    version: u32,
    #[serde(default)]
    items: Vec<ActivityItemDto>,
}

fn parse_stored(raw: &str) -> Vec<ActivityItemDto> {
    if raw.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str::<StoredRecentActivity>(raw)
        .map(|stored| {
            stored
                .items
                .into_iter()
                .filter(|item| {
                    !item.id.trim().is_empty()
                        && !item.message.trim().is_empty()
                        && !item.timestamp.trim().is_empty()
                        && matches!(item.activity_type.as_str(), "runtime" | "model")
                })
                .take(MAX_ITEMS)
                .collect()
        })
        .unwrap_or_else(|_| {
            // Legacy: bare array (localStorage shape).
            serde_json::from_str::<Vec<ActivityItemDto>>(raw)
                .unwrap_or_default()
                .into_iter()
                .filter(|item| matches!(item.activity_type.as_str(), "runtime" | "model"))
                .take(MAX_ITEMS)
                .collect()
        })
}

fn serialize_stored(items: &[ActivityItemDto]) -> CommandResult<String> {
    let stored = StoredRecentActivity {
        version: 1,
        items: items.to_vec(),
    };
    serde_json::to_string(&stored).map_err(|err| {
        CommandError::from(promptlab_core::PromptLabError::internal(err.to_string()))
    })
}

async fn load_items(state: &AppState) -> CommandResult<Vec<ActivityItemDto>> {
    let record = state
        .repositories()
        .recent_activity()
        .get()
        .await
        .map_err(CommandError::from)?;
    Ok(record
        .map(|row| parse_stored(&row.data_json))
        .unwrap_or_default())
}

async fn save_items(state: &AppState, items: &[ActivityItemDto]) -> CommandResult<()> {
    let raw = serialize_stored(items)?;
    state
        .repositories()
        .recent_activity()
        .upsert(&raw)
        .await
        .map_err(CommandError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn activity_list(state: State<'_, AppState>) -> CommandResult<Vec<ActivityItemDto>> {
    load_items(state.inner()).await
}

#[tauri::command]
pub async fn activity_record(
    state: State<'_, AppState>,
    request: ActivityRecordRequest,
) -> CommandResult<ActivityItemDto> {
    let activity_type = request.activity_type.trim().to_ascii_lowercase();
    if !matches!(activity_type.as_str(), "runtime" | "model") {
        return Err(CommandError::invalid_input(
            "activity type must be runtime or model",
        ));
    }
    let message = request.message.trim();
    if message.is_empty() {
        return Err(CommandError::invalid_input("activity message is required"));
    }

    let timestamp = OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| OffsetDateTime::now_utc().to_string());
    let item = ActivityItemDto {
        id: request
            .id
            .filter(|id| !id.trim().is_empty())
            .unwrap_or_else(|| format!("local-{activity_type}-{timestamp}")),
        activity_type,
        message: message.to_string(),
        timestamp,
    };

    let mut items = load_items(state.inner()).await?;
    items.retain(|existing| existing.id != item.id);
    items.insert(0, item.clone());
    items.truncate(MAX_ITEMS);
    save_items(state.inner(), &items).await?;
    Ok(item)
}

#[tauri::command]
pub async fn activity_replace_all(
    state: State<'_, AppState>,
    items: Vec<ActivityItemDto>,
) -> CommandResult<Vec<ActivityItemDto>> {
    let cleaned: Vec<ActivityItemDto> = items
        .into_iter()
        .filter(|item| {
            !item.id.trim().is_empty()
                && !item.message.trim().is_empty()
                && !item.timestamp.trim().is_empty()
                && matches!(item.activity_type.as_str(), "runtime" | "model")
        })
        .take(MAX_ITEMS)
        .collect();
    save_items(state.inner(), &cleaned).await?;
    Ok(cleaned)
}
