use std::path::{Path, PathBuf};

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

use crate::error::{PluginError, PluginResult};
use crate::types::{PluginHooks, PluginLanguage, PluginPermissions, PluginType};

pub const MANIFEST_FILE: &str = "promptlab-plugin.toml";
pub const HOST_API_VERSION: &str = "1";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginManifest {
    pub plugin: PluginSection,
    pub runtime: RuntimeSection,
    #[serde(default)]
    pub capabilities: PluginPermissions,
    #[serde(default)]
    pub hooks: PluginHooks,
    #[serde(default)]
    pub permissions: PermissionsRationale,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginSection {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: String,
    #[serde(default)]
    pub plugin_type: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuntimeSection {
    #[serde(default = "default_runtime_type")]
    pub r#type: String,
    pub entry: String,
    #[serde(default)]
    pub interpreter: Option<String>,
    #[serde(default)]
    pub min_promptlab: Option<String>,
}

fn default_runtime_type() -> String {
    "subprocess".into()
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PermissionsRationale {
    #[serde(flatten)]
    pub entries: std::collections::HashMap<String, String>,
}

impl PluginManifest {
    pub fn load(path: impl AsRef<Path>) -> PluginResult<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| PluginError::InvalidManifest(e.to_string()))?;
        Self::parse(&content)
    }

    pub fn parse(content: &str) -> PluginResult<Self> {
        let manifest: PluginManifest = toml::from_str(content)
            .map_err(|e| PluginError::InvalidManifest(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> PluginResult<()> {
        if self.plugin.id.is_empty() {
            return Err(PluginError::InvalidManifest("plugin.id is required".into()));
        }
        Version::parse(&self.plugin.version)
            .map_err(|e| PluginError::InvalidManifest(format!("invalid version: {e}")))?;
        if self.plugin.api_version != HOST_API_VERSION {
            return Err(PluginError::InvalidManifest(format!(
                "unsupported api_version {} (host supports {HOST_API_VERSION})",
                self.plugin.api_version
            )));
        }
        if self.runtime.r#type != "subprocess" {
            return Err(PluginError::InvalidManifest(format!(
                "unsupported runtime type: {}",
                self.runtime.r#type
            )));
        }
        if let Some(min) = &self.runtime.min_promptlab {
            let req = VersionReq::parse(min)
                .map_err(|e| PluginError::InvalidManifest(format!("invalid min_promptlab: {e}")))?;
            let host = Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_else(|_| Version::new(0, 1, 0));
            if !req.matches(&host) {
                return Err(PluginError::VersionIncompatible(format!(
                    "plugin requires promptlab {min}, host is {host}"
                )));
            }
        }
        Ok(())
    }

    pub fn plugin_type(&self) -> PluginResult<PluginType> {
        parse_plugin_type(
            self.plugin
                .plugin_type
                .as_deref()
                .ok_or_else(|| PluginError::InvalidManifest("plugin_type is required".into()))?,
        )
    }

    pub fn language(&self) -> PluginResult<PluginLanguage> {
        parse_language(
            self.plugin
                .language
                .as_deref()
                .ok_or_else(|| PluginError::InvalidManifest("language is required".into()))?,
        )
    }

    pub fn entry_path(&self, plugin_dir: &Path) -> PathBuf {
        plugin_dir.join(&self.runtime.entry)
    }

    pub fn interpreter(&self) -> PluginResult<String> {
        if let Some(interp) = &self.runtime.interpreter {
            return Ok(interp.clone());
        }
        Ok(self.language()?.default_interpreter().to_string())
    }
}

pub fn parse_plugin_type(raw: &str) -> PluginResult<PluginType> {
    match raw.to_lowercase().as_str() {
        "discovery" => Ok(PluginType::Discovery),
        "attack" => Ok(PluginType::Attack),
        "judge" => Ok(PluginType::Judge),
        "report" => Ok(PluginType::Report),
        other => Err(PluginError::InvalidManifest(format!(
            "unknown plugin_type: {other}"
        ))),
    }
}

pub fn parse_language(raw: &str) -> PluginResult<PluginLanguage> {
    match raw.to_lowercase().as_str() {
        "python" | "py" => Ok(PluginLanguage::Python),
        "javascript" | "js" | "node" => Ok(PluginLanguage::JavaScript),
        other => Err(PluginError::InvalidManifest(format!(
            "unsupported language: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_manifest() {
        let toml = r#"
[plugin]
id = "com.test.plugin"
name = "Test"
version = "1.0.0"
api_version = "1"
plugin_type = "discovery"
language = "python"

[runtime]
type = "subprocess"
entry = "plugin.py"

[capabilities]
finding_emit = true
log = true

[hooks]
discover = "discover"
"#;
        let m = PluginManifest::parse(toml).unwrap();
        assert_eq!(m.plugin.id, "com.test.plugin");
        assert!(m.capabilities.finding_emit);
    }
}
