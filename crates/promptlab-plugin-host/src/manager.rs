use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tracing::{info, instrument};

use crate::error::{PluginError, PluginResult};
use crate::lifecycle::PluginLifecycle;
use crate::manifest::{PluginManifest, MANIFEST_FILE};
use crate::permissions::PermissionGuard;
use crate::sandbox::SandboxRunner;
use crate::types::{
    PluginInvokeResult, PluginLanguage, PluginRecord, PluginState, PluginType, SandboxConfig,
};

/// Discovers, installs, and executes PromptLab plugins.
pub struct PluginManager {
    plugins_dir: PathBuf,
    records: HashMap<String, PluginRecord>,
    lifecycles: HashMap<String, PluginLifecycle>,
    sandbox: SandboxRunner,
}

impl PluginManager {
    pub fn new(plugins_dir: impl AsRef<Path>) -> PluginResult<Self> {
        let plugins_dir = plugins_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&plugins_dir)?;
        Ok(Self {
            plugins_dir,
            records: HashMap::new(),
            lifecycles: HashMap::new(),
            sandbox: SandboxRunner::with_defaults(),
        })
    }

    pub fn with_sandbox(plugins_dir: impl AsRef<Path>, config: SandboxConfig) -> PluginResult<Self> {
        let mut mgr = Self::new(plugins_dir)?;
        mgr.sandbox = SandboxRunner::new(config);
        Ok(mgr)
    }

    pub fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }

    pub fn list(&self) -> Vec<&PluginRecord> {
        let mut items: Vec<_> = self.records.values().collect();
        items.sort_by(|a, b| a.name.cmp(&b.name));
        items
    }

    pub fn get(&self, id: &str) -> Option<&PluginRecord> {
        self.records.get(id)
    }

    pub fn state(&self, id: &str) -> Option<PluginState> {
        self.lifecycles.get(id).map(|l| l.state())
    }

    /// Scan plugins directory recursively for manifests.
    pub fn discover(&mut self) -> PluginResult<Vec<String>> {
        let mut discovered = Vec::new();
        self.discover_dir(&self.plugins_dir.clone(), &mut discovered)?;
        info!(count = discovered.len(), "plugins discovered");
        Ok(discovered)
    }

    fn discover_dir(&mut self, dir: &Path, discovered: &mut Vec<String>) -> PluginResult<()> {
        let manifest_path = dir.join(MANIFEST_FILE);
        if manifest_path.exists() {
            let manifest = PluginManifest::load(&manifest_path)?;
            let id = manifest.plugin.id.clone();
            if !self.records.contains_key(&id) {
                self.register_discovered(manifest, dir.to_path_buf())?;
                discovered.push(id);
            }
            return Ok(());
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };

        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('_') || name == "node_modules" {
                continue;
            }
            self.discover_dir(&entry.path(), discovered)?;
        }
        Ok(())
    }

    fn register_discovered(&mut self, manifest: PluginManifest, dir: PathBuf) -> PluginResult<()> {
        let plugin_type = manifest.plugin_type()?;
        let language = manifest.language()?;
        let id = manifest.plugin.id.clone();

        let record = PluginRecord {
            id: id.clone(),
            name: manifest.plugin.name.clone(),
            version: manifest.plugin.version.clone(),
            api_version: manifest.plugin.api_version.clone(),
            plugin_type,
            language,
            install_path: dir,
            state: PluginState::Discovered,
            permissions: manifest.capabilities.clone(),
            hooks: manifest.hooks.clone(),
            enabled: false,
        };

        self.records.insert(id.clone(), record);
        self.lifecycles.insert(id.clone(), PluginLifecycle::new(&id));
        if let Some(lc) = self.lifecycles.get_mut(&id) {
            lc.transition(PluginState::Installed, Some("installed from disk".into()))?;
        }
        if let Some(rec) = self.records.get_mut(&id) {
            rec.state = PluginState::Installed;
        }
        Ok(())
    }

    /// Enable a plugin for execution.
    pub fn enable(&mut self, id: &str) -> PluginResult<()> {
        let lc = self
            .lifecycles
            .get_mut(id)
            .ok_or_else(|| PluginError::not_found(id))?;
        lc.transition(PluginState::Enabled, None)?;
        if let Some(rec) = self.records.get_mut(id) {
            rec.enabled = true;
            rec.state = PluginState::Enabled;
        }
        Ok(())
    }

    /// Disable a plugin.
    pub fn disable(&mut self, id: &str) -> PluginResult<()> {
        if let Some(lc) = self.lifecycles.get_mut(id) {
            if lc.state() == PluginState::Loaded || lc.state() == PluginState::Active {
                lc.transition(PluginState::Disabled, Some("disabled while loaded".into()))?;
            } else {
                lc.transition(PluginState::Disabled, None)?;
            }
        }
        if let Some(rec) = self.records.get_mut(id) {
            rec.enabled = false;
            rec.state = PluginState::Disabled;
        }
        Ok(())
    }

    /// Invoke the plugin's primary hook for its type.
    #[instrument(skip(self, params), fields(plugin_id = %id))]
    pub async fn invoke(
        &mut self,
        id: &str,
        params: serde_json::Value,
    ) -> PluginResult<PluginInvokeResult> {
        let record = self
            .records
            .get(id)
            .ok_or_else(|| PluginError::not_found(id))?
            .clone();

        if !record.enabled {
            return Err(PluginError::Lifecycle(format!("plugin {id} is not enabled")));
        }

        let hook = record
            .hooks
            .hook_for(record.plugin_type)
            .unwrap_or(record.plugin_type.default_hook())
            .to_string();

        self.invoke_hook(id, &hook, params).await
    }

    /// Invoke a specific hook by name.
    pub async fn invoke_hook(
        &mut self,
        id: &str,
        hook: &str,
        params: serde_json::Value,
    ) -> PluginResult<PluginInvokeResult> {
        let record = self
            .records
            .get(id)
            .ok_or_else(|| PluginError::not_found(id))?
            .clone();

        if let Some(lc) = self.lifecycles.get_mut(id) {
            lc.transition(PluginState::Loaded, None)?;
        }
        if let Some(rec) = self.records.get_mut(id) {
            rec.state = PluginState::Loaded;
        }

        let manifest_path = record.install_path.join(MANIFEST_FILE);
        let manifest = PluginManifest::load(&manifest_path)?;
        let guard = PermissionGuard::new(record.permissions.clone());

        let result = self
            .sandbox
            .invoke(&manifest, &record.install_path, hook, params, &guard)
            .await;

        if let Some(lc) = self.lifecycles.get_mut(id) {
            if result.is_ok() {
                let _ = lc.transition(PluginState::Active, None);
                let _ = lc.transition(PluginState::Loaded, None);
            } else {
                let _ = lc.fail("invoke failed");
            }
        }

        result
    }

    /// List plugins filtered by type.
    pub fn by_type(&self, plugin_type: PluginType) -> Vec<&PluginRecord> {
        self.records
            .values()
            .filter(|r| r.plugin_type == plugin_type)
            .collect()
    }

    /// List plugins filtered by language.
    pub fn by_language(&self, language: PluginLanguage) -> Vec<&PluginRecord> {
        self.records
            .values()
            .filter(|r| r.language == language)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manager_discovers_plugins_dir() {
        let dir = tempfile::tempdir().unwrap();
        let sample = dir.path().join("sample-discovery");
        std::fs::create_dir_all(&sample).unwrap();
        std::fs::write(
            sample.join(MANIFEST_FILE),
            r#"
[plugin]
id = "com.promptlab.sample.discovery"
name = "Sample Discovery"
version = "1.0.0"
api_version = "1"
plugin_type = "discovery"
language = "python"

[runtime]
type = "subprocess"
entry = "plugin.py"

[capabilities]
log = true

[hooks]
discover = "discover"
"#,
        )
        .unwrap();
        std::fs::write(sample.join("plugin.py"), "print('stub')").unwrap();

        let mut mgr = PluginManager::new(dir.path()).unwrap();
        let ids = mgr.discover().unwrap();
        assert_eq!(ids.len(), 1);
        mgr.enable(&ids[0]).unwrap();
        assert!(mgr.get(&ids[0]).unwrap().enabled);
    }
}
