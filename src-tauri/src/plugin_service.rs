//! Plugin manager bootstrap (leftover — no longer seeds or creates a plugins dir).

use std::path::{Path, PathBuf};

use promptlab_plugin_host::{persist_enabled, PluginManager, PluginResult};

pub fn plugins_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("plugins")
}

/// Empty plugin manager — does not create `~/.promptlab/plugins`.
pub fn bootstrap_plugin_manager(data_dir: &Path) -> PluginResult<PluginManager> {
    PluginManager::new(plugins_dir(data_dir))
}

pub fn save_plugin_state(manager: &PluginManager, data_dir: &Path) -> PluginResult<()> {
    persist_enabled(manager, data_dir)
}
