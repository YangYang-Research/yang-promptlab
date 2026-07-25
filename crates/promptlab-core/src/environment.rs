//! PromptLab root directory layout, environment configuration, and startup validation.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{PromptLabError, PromptLabResult};

pub const ROOT_DIR_NAME: &str = ".promptlab";
pub const ENV_CONFIG_FILE: &str = "environment.json";
pub const DB_FILENAME: &str = "promptlab.db";
pub const DB_PATH_ENV: &str = "PROMPTLAB_DB_PATH";
pub const ROOT_PATH_ENV: &str = "PROMPTLAB_ROOT";

/// Resolved directory layout under the PromptLab root.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentPaths {
    pub root: PathBuf,
    pub config: PathBuf,
    pub workspaces: PathBuf,
    pub models: PathBuf,
    pub runtime: PathBuf,
    pub logs: PathBuf,
    pub plugins: PathBuf,
    pub cache: PathBuf,
    pub temp: PathBuf,
    pub backups: PathBuf,
}

impl EnvironmentPaths {
    pub fn database_path(&self) -> PathBuf {
        self.workspaces.join(DB_FILENAME)
    }

    pub fn reports_dir(&self) -> PathBuf {
        self.workspaces.join("reports")
    }

    pub fn auth_sessions_dir(&self) -> PathBuf {
        self.workspaces.join("AuthSessions")
    }

    pub fn inference_config_path(&self) -> PathBuf {
        self.config.join("ai_runtime_config.json")
    }

    pub fn proxy_settings_path(&self) -> PathBuf {
        self.config.join(crate::proxy::PROXY_SETTINGS_FILE)
    }

    pub fn plugins_state_path(&self) -> PathBuf {
        self.config.join("plugins_state.json")
    }

    pub fn environment_config_path(&self) -> PathBuf {
        self.config.join(ENV_CONFIG_FILE)
    }

    pub fn all_dirs(&self) -> [&Path; 9] {
        [
            &self.config,
            &self.workspaces,
            &self.models,
            &self.runtime,
            &self.logs,
            &self.plugins,
            &self.cache,
            &self.temp,
            &self.backups,
        ]
    }
}

/// Persisted environment overrides (paths relative to root unless absolute).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspaces: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logs: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugins: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temp: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backups: Option<PathBuf>,
}

/// Default root: `~/.promptlab` (macOS/Linux) or `%USERPROFILE%\.promptlab` (Windows).
pub fn default_root_dir() -> PathBuf {
    if let Ok(custom) = std::env::var(ROOT_PATH_ENV) {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    user_home().join(ROOT_DIR_NAME)
}

pub fn user_home() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(profile) = std::env::var("USERPROFILE") {
            if !profile.trim().is_empty() {
                return PathBuf::from(profile);
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.trim().is_empty() {
            return PathBuf::from(home);
        }
    }
    PathBuf::from(".")
}

fn resolve_subdir(root: &Path, override_path: Option<&PathBuf>, default_name: &str) -> PathBuf {
    match override_path {
        Some(path) if path.is_absolute() => path.clone(),
        Some(path) => root.join(path),
        None => root.join(default_name),
    }
}

pub fn resolve_paths(config: &EnvironmentConfig) -> EnvironmentPaths {
    let root = config
        .root
        .clone()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(default_root_dir);

    EnvironmentPaths {
        config: root.join("config"),
        workspaces: resolve_subdir(&root, config.workspaces.as_ref(), "workspaces"),
        models: resolve_subdir(&root, config.models.as_ref(), "models"),
        runtime: resolve_subdir(&root, config.runtime.as_ref(), "runtime"),
        logs: resolve_subdir(&root, config.logs.as_ref(), "logs"),
        plugins: resolve_subdir(&root, config.plugins.as_ref(), "plugins"),
        cache: resolve_subdir(&root, config.cache.as_ref(), "cache"),
        temp: resolve_subdir(&root, config.temp.as_ref(), "temp"),
        backups: resolve_subdir(&root, config.backups.as_ref(), "backups"),
        root,
    }
}

pub fn load_environment_config(root: &Path) -> EnvironmentConfig {
    let path = root.join("config").join(ENV_CONFIG_FILE);
    if !path.is_file() {
        return EnvironmentConfig {
            root: Some(root.to_path_buf()),
            ..Default::default()
        };
    }
    let raw = fs::read_to_string(&path).unwrap_or_default();
    serde_json::from_str(&raw).unwrap_or_else(|_| EnvironmentConfig {
        root: Some(root.to_path_buf()),
        ..Default::default()
    })
}

pub fn save_environment_config(paths: &EnvironmentPaths, config: &EnvironmentConfig) -> PromptLabResult<()> {
    fs::create_dir_all(&paths.config).map_err(PromptLabError::from)?;
    let raw = serde_json::to_string_pretty(config).map_err(|e| PromptLabError::internal(e.to_string()))?;
    let file = paths.environment_config_path();
    let tmp = file.with_extension("json.tmp");
    fs::write(&tmp, raw).map_err(PromptLabError::from)?;
    fs::rename(&tmp, &file).map_err(PromptLabError::from)?;
    Ok(())
}

/// Load config, resolve paths, create missing directories, verify permissions.
pub fn bootstrap_environment() -> PromptLabResult<EnvironmentPaths> {
    let root = default_root_dir();
    let mut config = load_environment_config(&root);
    if config.root.is_none() {
        config.root = Some(root.clone());
    }

    let paths = resolve_paths(&config);
    ensure_environment(&paths)?;
    save_environment_config(&paths, &config)?;
    Ok(paths)
}

pub fn ensure_environment(paths: &EnvironmentPaths) -> PromptLabResult<()> {
    fs::create_dir_all(&paths.root).map_err(|e| {
        PromptLabError::config(format!(
            "cannot create PromptLab root directory {}: {e}",
            paths.root.display()
        ))
    })?;

    for dir in paths.all_dirs() {
        fs::create_dir_all(dir).map_err(|e| {
            PromptLabError::config(format!(
                "cannot create required directory {}: {e}",
                dir.display()
            ))
        })?;
    }

    verify_writable(&paths.root)?;
    verify_writable(&paths.workspaces)?;
    verify_writable(&paths.logs)?;

    fs::create_dir_all(paths.reports_dir()).map_err(PromptLabError::from)?;
    fs::create_dir_all(paths.auth_sessions_dir()).map_err(PromptLabError::from)?;

    Ok(())
}

fn verify_writable(path: &Path) -> PromptLabResult<()> {
    if !path.exists() {
        return Err(PromptLabError::config(format!(
            "directory does not exist: {}",
            path.display()
        )));
    }
    let probe = path.join(".promptlab_write_probe");
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&probe)
        .map_err(|e| {
            PromptLabError::config(format!(
                "no write permission for {}: {e}",
                path.display()
            ))
        })?;
    file.write_all(b"ok").map_err(PromptLabError::from)?;
    drop(file);
    let _ = fs::remove_file(probe);
    Ok(())
}

pub fn resolve_db_path(workspaces_dir: &Path) -> PathBuf {
    match std::env::var(DB_PATH_ENV) {
        Ok(custom) if !custom.trim().is_empty() => PathBuf::from(custom.trim()),
        _ => workspaces_dir.join(DB_FILENAME),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_root_uses_home() {
        let root = default_root_dir();
        assert!(root.ends_with(ROOT_DIR_NAME));
    }

    #[test]
    fn resolve_paths_under_root() {
        let config = EnvironmentConfig::default();
        let paths = resolve_paths(&config);
        assert!(paths.models.starts_with(&paths.root));
        assert!(paths.workspaces.join(DB_FILENAME) == paths.database_path());
    }
}
