//! Persist AI Runtime token usage totals (per agent) in SQLite `token_usage`.

use std::collections::BTreeMap;

use promptlab_inference::{
    AgentTokenUsage, TokenUsageSnapshot, token_usage_export_map, token_usage_migrate_unattributed,
    token_usage_replace_all, token_usage_reset, token_usage_snapshot, token_usage_take_dirty,
};
use promptlab_storage::{Database, JsonDocumentRepository};
use tauri::{AppHandle, Manager};
use tracing::{info, warn};

use crate::state::AppState;

const FLUSH_INTERVAL_MS: u64 = 2_000;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct StoredTokenUsage {
    version: u32,
    #[serde(default)]
    agents: BTreeMap<String, AgentTokenUsage>,
}

fn parse_stored(raw: &str) -> Result<BTreeMap<String, AgentTokenUsage>, String> {
    if raw.trim().is_empty() {
        return Ok(BTreeMap::new());
    }
    let stored: StoredTokenUsage = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    Ok(stored.agents)
}

fn serialize_stored(map: &BTreeMap<String, AgentTokenUsage>) -> Result<String, String> {
    let stored = StoredTokenUsage {
        version: 1,
        agents: map.clone(),
    };
    serde_json::to_string(&stored).map_err(|e| e.to_string())
}

async fn load_from_db(db: &Database) -> Result<Option<BTreeMap<String, AgentTokenUsage>>, String> {
    let record = db
        .repositories()
        .token_usage()
        .get()
        .await
        .map_err(|e| e.to_string())?;
    match record {
        Some(row) => Ok(Some(parse_stored(&row.data_json)?)),
        None => Ok(None),
    }
}

async fn save_to_db(db: &Database, map: &BTreeMap<String, AgentTokenUsage>) -> Result<(), String> {
    let raw = serialize_stored(map)?;
    db.repositories()
        .token_usage()
        .upsert(&raw)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn bootstrap_token_usage_persistence(app: &AppHandle) {
    let state = app.state::<AppState>();
    let db = state.database().clone();

    match hydrate_from_db(&db).await {
        Ok(()) => info!("hydrated AI runtime token usage from SQLite"),
        Err(err) => warn!(error = %err, "token usage hydrate skipped"),
    }

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(FLUSH_INTERVAL_MS)).await;
            let state = app_handle.state::<AppState>();
            if let Err(err) = flush_token_usage(state.database()).await {
                warn!(error = %err, "token usage flush failed");
            }
        }
    });
}

async fn hydrate_from_db(db: &Database) -> Result<(), String> {
    if let Some(map) = load_from_db(db).await? {
        token_usage_replace_all(map);
        token_usage_migrate_unattributed();
    }
    Ok(())
}

pub async fn flush_token_usage(db: &Database) -> Result<(), String> {
    let Some(map) = token_usage_take_dirty() else {
        return Ok(());
    };
    save_to_db(db, &map).await
}

pub async fn usage_snapshot(db: &Database) -> TokenUsageSnapshot {
    let _ = flush_token_usage(db).await;
    token_usage_snapshot()
}

pub async fn reset_usage(db: &Database) -> Result<TokenUsageSnapshot, String> {
    token_usage_reset();
    save_to_db(db, &token_usage_export_map()).await?;
    Ok(token_usage_snapshot())
}
