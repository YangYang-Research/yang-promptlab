//! Persist AI Runtime token usage totals (per agent) to JSON under the data dir.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use promptlab_inference::{
    AgentTokenUsage, TokenUsageSnapshot, token_usage_export_map, token_usage_migrate_unattributed,
    token_usage_replace_all, token_usage_reset, token_usage_snapshot, token_usage_take_dirty,
};
use tauri::{AppHandle, Manager};
use tracing::{info, warn};

use crate::state::AppState;

const FLUSH_INTERVAL_MS: u64 = 2_000;

fn usage_path(data_dir: &Path) -> PathBuf {
    data_dir.join("config").join("token_usage.json")
}

pub async fn bootstrap_token_usage_persistence(app: &AppHandle) {
    let state = app.state::<AppState>();
    let path = usage_path(state.data_dir());
    match load_usage_file(&path) {
        Ok(map) => {
            token_usage_replace_all(map);
            token_usage_migrate_unattributed();
            info!(path = %path.display(), "hydrated AI runtime token usage");
        }
        Err(err) => warn!(error = %err, path = %path.display(), "token usage hydrate skipped"),
    }

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(FLUSH_INTERVAL_MS)).await;
            let state = app_handle.state::<AppState>();
            if let Err(err) = flush_token_usage(state.data_dir()) {
                warn!(error = %err, "token usage flush failed");
            }
        }
    });
}

pub fn flush_token_usage(data_dir: &Path) -> Result<(), String> {
    let Some(map) = token_usage_take_dirty() else {
        return Ok(());
    };
    save_usage_file(&usage_path(data_dir), &map)
}

pub fn usage_snapshot(data_dir: &Path) -> TokenUsageSnapshot {
    let _ = flush_token_usage(data_dir);
    token_usage_snapshot()
}

pub fn reset_usage(data_dir: &Path) -> Result<TokenUsageSnapshot, String> {
    token_usage_reset();
    save_usage_file(&usage_path(data_dir), &token_usage_export_map())?;
    Ok(token_usage_snapshot())
}

fn load_usage_file(path: &Path) -> Result<BTreeMap<String, AgentTokenUsage>, String> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    if raw.trim().is_empty() {
        return Ok(BTreeMap::new());
    }
    let stored: StoredTokenUsage = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    Ok(stored.agents)
}

fn save_usage_file(path: &Path, map: &BTreeMap<String, AgentTokenUsage>) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let stored = StoredTokenUsage {
        version: 1,
        agents: map.clone(),
    };
    let raw = serde_json::to_string_pretty(&stored).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct StoredTokenUsage {
    version: u32,
    #[serde(default)]
    agents: BTreeMap<String, AgentTokenUsage>,
}
