//! Plugin manager bootstrap and sample seeding.

use std::fs;
use std::path::{Path, PathBuf};

use promptlab_plugin_host::{
    persist_enabled, restore_enabled, PluginManager, PluginResult,
};

pub fn plugins_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("plugins")
}

fn bundled_samples_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("PROMPTLAB_PLUGINS_SAMPLES") {
        let path = PathBuf::from(dir);
        if path.is_dir() {
            return Some(path);
        }
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../plugins/samples");
    if repo.is_dir() {
        return repo.canonicalize().ok();
    }
    None
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Seed bundled sample plugins when the plugins directory is empty.
pub fn seed_samples_if_empty(data_dir: &Path) -> PluginResult<()> {
    let dir = plugins_dir(data_dir);
    fs::create_dir_all(&dir)?;

    let has_plugins = fs::read_dir(&dir)
        .map(|mut entries| entries.any(|e| e.is_ok()))
        .unwrap_or(false);
    if has_plugins {
        return Ok(());
    }

    let Some(samples) = bundled_samples_dir() else {
        return Ok(());
    };
    copy_dir_all(&samples, &dir)?;
    tracing::info!(path = %dir.display(), "seeded bundled plugin samples");
    Ok(())
}

/// Discover plugins on disk and restore enabled state.
pub fn bootstrap_plugin_manager(data_dir: &Path) -> PluginResult<PluginManager> {
    seed_samples_if_empty(data_dir)?;
    let mut manager = PluginManager::new(plugins_dir(data_dir))?;
    let _ = manager.discover();
    restore_enabled(&mut manager, data_dir)?;
    Ok(manager)
}

pub fn save_plugin_state(manager: &PluginManager, data_dir: &Path) -> PluginResult<()> {
    persist_enabled(manager, data_dir)
}
