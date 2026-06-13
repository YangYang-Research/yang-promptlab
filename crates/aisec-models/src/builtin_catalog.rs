use std::path::{Path, PathBuf};

use serde::Deserialize;
use tracing::{debug, warn};

use crate::error::{ModelError, ModelResult};
use crate::types::{ModelCapabilities, ModelCatalogEntry, ModelProvider};

#[derive(Debug, Clone, Deserialize)]
struct RegistryFile {
    models: Vec<BuiltinRegistryEntry>,
}

/// Entry shape from `resources/models.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltinRegistryEntry {
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
pub struct BuiltinCatalogMeta {
    pub source_path: Option<PathBuf>,
    pub entry_count: usize,
    pub remote_merged: bool,
    pub remote_url: Option<String>,
}

pub struct BuiltinCatalog {
    entries: Vec<ModelCatalogEntry>,
    raw: Vec<BuiltinRegistryEntry>,
    meta: BuiltinCatalogMeta,
}

impl BuiltinCatalog {
    pub fn load_from_path(path: &Path) -> ModelResult<Self> {
        let data = std::fs::read_to_string(path).map_err(ModelError::Io)?;
        let file: RegistryFile =
            serde_json::from_str(&data).map_err(|e| ModelError::invalid(e.to_string()))?;
        let raw = file.models;
        let entries: Vec<_> = raw.iter().map(entry_to_catalog).collect();
        let entry_count = entries.len();
        Ok(Self {
            entries,
            raw,
            meta: BuiltinCatalogMeta {
                source_path: Some(path.to_path_buf()),
                entry_count,
                remote_merged: false,
                remote_url: None,
            },
        })
    }

    pub async fn load_with_optional_remote(
        bundled_path: &Path,
        remote_url: Option<&str>,
    ) -> ModelResult<Self> {
        let mut catalog = if bundled_path.exists() {
            Self::load_from_path(bundled_path)?
        } else {
            Self {
                entries: Vec::new(),
                raw: Vec::new(),
                meta: BuiltinCatalogMeta::default(),
            }
        };

        if let Some(url) = remote_url.filter(|u| !u.trim().is_empty()) {
            match merge_remote(&mut catalog.raw, url).await {
                Ok(added) if added > 0 => {
                    catalog.entries = catalog.raw.iter().map(entry_to_catalog).collect();
                    catalog.meta.entry_count = catalog.entries.len();
                    catalog.meta.remote_merged = true;
                    catalog.meta.remote_url = Some(url.to_string());
                }
                Ok(_) => {
                    catalog.meta.remote_url = Some(url.to_string());
                }
                Err(err) => {
                    warn!(%url, error = %err, "optional registry merge failed; continuing offline");
                    catalog.meta.remote_url = Some(url.to_string());
                }
            }
        }

        Ok(catalog)
    }

    pub fn entries(&self) -> &[ModelCatalogEntry] {
        &self.entries
    }

    pub fn find(&self, id: &str) -> Option<&ModelCatalogEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn find_raw(&self, id: &str) -> Option<&BuiltinRegistryEntry> {
        self.raw.iter().find(|e| e.id == id)
    }

    pub fn meta(&self) -> &BuiltinCatalogMeta {
        &self.meta
    }
}

async fn merge_remote(entries: &mut Vec<BuiltinRegistryEntry>, url: &str) -> ModelResult<usize> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| ModelError::download(e.to_string()))?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| ModelError::download(e.to_string()))?;
    if !response.status().is_success() {
        return Err(ModelError::download(format!(
            "remote registry status {}",
            response.status()
        )));
    }
    let remote: RegistryFile = response
        .json()
        .await
        .map_err(|e| ModelError::download(e.to_string()))?;

    let mut added = 0usize;
    for entry in remote.models {
        if entries.iter().any(|e| e.id == entry.id) {
            continue;
        }
        debug!(id = %entry.id, "merged remote registry entry");
        entries.push(entry);
        added += 1;
    }
    Ok(added)
}

pub fn entry_to_catalog(entry: &BuiltinRegistryEntry) -> ModelCatalogEntry {
    let provider = parse_provider(&entry.provider);
    let capabilities = match provider {
        ModelProvider::Ollama => ModelCapabilities::ollama(),
        _ => ModelCapabilities::gguf(),
    };
    let version = entry
        .file
        .clone()
        .or_else(|| entry.ollama_tag.clone())
        .unwrap_or_else(|| entry.id.clone());
    let quant = entry
        .file
        .as_ref()
        .and_then(|f| infer_quant(f));
    let description = if entry.recommended {
        format!("{} · {} (recommended)", entry.name, entry.purpose)
    } else {
        format!("{} · {}", entry.name, entry.purpose)
    };

    ModelCatalogEntry {
        id: entry.id.clone(),
        name: entry.name.clone(),
        provider,
        version,
        description,
        purpose: entry.purpose.clone(),
        recommended: entry.recommended,
        size_bytes: None,
        quant,
        capabilities,
        repo: entry.repo.clone(),
        filename: entry.file.clone(),
        ollama_tag: entry.ollama_tag.clone(),
    }
}

fn parse_provider(value: &str) -> ModelProvider {
    match value.to_ascii_lowercase().as_str() {
        "ollama" => ModelProvider::Ollama,
        "huggingface" | "hf" => ModelProvider::HuggingFace,
        _ => ModelProvider::Gguf,
    }
}

fn infer_quant(filename: &str) -> Option<String> {
    let upper = filename.to_ascii_uppercase();
    for token in ["Q8_0", "Q6_K", "Q5_K_M", "Q4_K_M", "Q4_0", "Q3_K_M", "Q2_K"] {
        if upper.contains(token) {
            return Some(token.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_registry_entry_to_catalog() {
        let entry = BuiltinRegistryEntry {
            id: "qwen3-8b-judge".into(),
            name: "Qwen3 8B".into(),
            purpose: "judge".into(),
            recommended: true,
            provider: "huggingface".into(),
            repo: Some("Qwen/Qwen3-8B-GGUF".into()),
            file: Some("qwen3-8b-q4_k_m.gguf".into()),
            ollama_tag: None,
            sha256: None,
        };
        let catalog = entry_to_catalog(&entry);
        assert_eq!(catalog.id, "qwen3-8b-judge");
        assert_eq!(catalog.provider, ModelProvider::HuggingFace);
        assert_eq!(catalog.quant.as_deref(), Some("Q4_K_M"));
    }
}
