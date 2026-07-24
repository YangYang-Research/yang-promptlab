//! Persist enabled plugin ids across restarts.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{PluginError, PluginResult};
use crate::manager::PluginManager;

const STATE_FILE: &str = "plugins_state.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PluginStateFile {
    #[serde(default)]
    enabled: Vec<String>,
}

pub fn state_file_path(data_dir: &Path) -> PathBuf {
    data_dir.join(STATE_FILE)
}

pub fn load_enabled_ids(data_dir: &Path) -> PluginResult<Vec<String>> {
    let path = state_file_path(data_dir);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path)?;
    let parsed: PluginStateFile = serde_json::from_str(&raw)
        .map_err(|err| PluginError::InvalidManifest(format!("invalid plugin state file: {err}")))?;
    Ok(parsed.enabled)
}

pub fn save_enabled_ids(data_dir: &Path, enabled: &[String]) -> PluginResult<()> {
    let path = state_file_path(data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let payload = PluginStateFile {
        enabled: enabled.to_vec(),
    };
    let json = serde_json::to_string_pretty(&payload)
        .map_err(|err| PluginError::InvalidManifest(err.to_string()))?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Re-enable plugins recorded in the state file.
pub fn restore_enabled(manager: &mut PluginManager, data_dir: &Path) -> PluginResult<()> {
    let enabled = load_enabled_ids(data_dir)?;
    let known: HashSet<String> = manager.list().into_iter().map(|r| r.id.clone()).collect();
    for id in enabled {
        if known.contains(&id) {
            let _ = manager.enable(&id);
        }
    }
    Ok(())
}

/// Snapshot currently enabled plugin ids to disk.
pub fn persist_enabled(manager: &PluginManager, data_dir: &Path) -> PluginResult<()> {
    let enabled: Vec<String> = manager
        .list()
        .into_iter()
        .filter(|record| record.enabled)
        .map(|record| record.id.clone())
        .collect();
    save_enabled_ids(data_dir, &enabled)
}
