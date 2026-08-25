use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::error::{RuntimeError, RuntimeResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryFile {
    pub models: Vec<RegistryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryEntry {
    pub id: String,
    pub name: String,
    pub purpose: String,
    #[serde(default)]
    pub recommended: bool,
    pub provider: String,
    #[serde(default)]
    pub repo: Option<String>,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub ollama_tag: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RegistryUpdateResult {
    pub merged_count: usize,
    pub remote_available: bool,
}

/// Legacy builtin model registry — no longer loads `models.json` (Remote-only).
pub struct BuiltinModelRegistry {
    entries: Vec<RegistryEntry>,
}

impl BuiltinModelRegistry {
    pub fn load_builtin(_app_root: impl AsRef<Path>) -> RuntimeResult<Self> {
        Ok(Self {
            entries: Vec::new(),
        })
    }

    pub fn load_from_path(path: &Path) -> RuntimeResult<Self> {
        let data = std::fs::read_to_string(path)
            .map_err(|err| RuntimeError::Registry(err.to_string()))?;
        let file: RegistryFile =
            serde_json::from_str(&data).map_err(|err| RuntimeError::Registry(err.to_string()))?;
        Ok(Self {
            entries: file.models,
        })
    }

    pub async fn load_with_optional_remote(
        app_root: impl AsRef<Path>,
        remote_url: Option<&str>,
    ) -> RuntimeResult<(Self, RegistryUpdateResult)> {
        let mut registry = Self::load_builtin(app_root)?;
        let mut result = RegistryUpdateResult {
            merged_count: registry.entries.len(),
            remote_available: false,
        };

        if let Some(url) = remote_url {
            match merge_remote(&mut registry.entries, url).await {
                Ok(count) => {
                    result.remote_available = true;
                    result.merged_count = count;
                }
                Err(err) => {
                    warn!(%url, error = %err, "optional registry update failed; continuing offline");
                }
            }
        }

        Ok((registry, result))
    }

    pub fn entries(&self) -> &[RegistryEntry] {
        &self.entries
    }

    pub fn find(&self, id: &str) -> Option<&RegistryEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }
}

async fn merge_remote(entries: &mut Vec<RegistryEntry>, url: &str) -> RuntimeResult<usize> {
    let client = promptlab_core::build_http_client(
        promptlab_core::HttpClientOptions::default()
            .with_timeout(std::time::Duration::from_secs(8)),
    )
    .map_err(|err| RuntimeError::Registry(err.to_string()))?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|err| RuntimeError::Registry(err.to_string()))?;
    if !response.status().is_success() {
        return Err(RuntimeError::Registry(format!(
            "remote registry status {}",
            response.status()
        )));
    }
    let remote: RegistryFile = response
        .json()
        .await
        .map_err(|err| RuntimeError::Registry(err.to_string()))?;

    for entry in remote.models {
        if entries.iter().any(|existing| existing.id == entry.id) {
            continue;
        }
        debug!(id = %entry.id, "merged remote registry entry");
        entries.push(entry);
    }

    Ok(entries.len())
}
