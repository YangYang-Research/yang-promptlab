use std::path::{Path, PathBuf};

use serde::Deserialize;
use tracing::{debug, info, warn};

use crate::download::huggingface_url;
use crate::error::{ModelError, ModelResult};
use crate::registry_validate::{validate_registry, RegistryValidationReport};
use crate::types::{ModelCapabilities, ModelCatalogEntry, ModelProvider};

#[derive(Debug, Clone, Deserialize)]
struct RegistryFile {
    models: Vec<BuiltinRegistryEntry>,
}

/// GGUF-first entry shape from `resources/models.json` (v2).
#[derive(Debug, Clone, Deserialize)]
pub struct BuiltinRegistryEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub recommended: bool,
    pub engine: String,
    pub format: String,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
    pub download_url: String,
}

#[derive(Debug, Clone, Default)]
pub struct BuiltinCatalogMeta {
    pub source_path: Option<PathBuf>,
    pub entry_count: usize,
    pub remote_merged: bool,
    pub remote_url: Option<String>,
    pub validation: RegistryValidationReport,
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
        Self::from_raw(file.models, Some(path.to_path_buf()), false, None)
    }

    pub async fn load_with_optional_remote(
        bundled_path: &Path,
        remote_url: Option<&str>,
    ) -> ModelResult<Self> {
        let mut raw = if bundled_path.exists() {
            let data = std::fs::read_to_string(bundled_path).map_err(ModelError::Io)?;
            let file: RegistryFile =
                serde_json::from_str(&data).map_err(|e| ModelError::invalid(e.to_string()))?;
            file.models
        } else {
            Vec::new()
        };

        let mut remote_merged = false;
        let mut merged_url = remote_url.map(str::to_string);

        if let Some(url) = remote_url.filter(|u| !u.trim().is_empty()) {
            match merge_remote(&mut raw, url).await {
                Ok(added) if added > 0 => {
                    remote_merged = true;
                    merged_url = Some(url.to_string());
                }
                Ok(_) => {
                    merged_url = Some(url.to_string());
                }
                Err(err) => {
                    warn!(%url, error = %err, "optional registry merge failed; continuing offline");
                    merged_url = Some(url.to_string());
                }
            }
        }

        Self::from_raw(
            raw,
            bundled_path.exists().then(|| bundled_path.to_path_buf()),
            remote_merged,
            merged_url,
        )
    }

    fn from_raw(
        raw: Vec<BuiltinRegistryEntry>,
        source_path: Option<PathBuf>,
        remote_merged: bool,
        remote_url: Option<String>,
    ) -> ModelResult<Self> {
        let (valid_raw, validation) = validate_registry(&raw);
        if validation.invalid > 0 {
            for issue in &validation.issues {
                warn!(
                    id = %issue.id,
                    field = %issue.field,
                    message = %issue.message,
                    "registry validation issue"
                );
            }
        }
        info!(
            total = validation.total,
            valid = validation.valid,
            invalid = validation.invalid,
            "validated built-in model registry"
        );

        let entries: Vec<_> = valid_raw.iter().map(entry_to_catalog).collect();
        let entry_count = entries.len();
        Ok(Self {
            entries,
            raw: valid_raw,
            meta: BuiltinCatalogMeta {
                source_path,
                entry_count,
                remote_merged,
                remote_url,
                validation,
            },
        })
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

    pub fn validation(&self) -> &RegistryValidationReport {
        &self.meta.validation
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
    let quant = infer_quant_from_url(&entry.download_url);
    let filename = filename_from_url(&entry.download_url);
    let format_label = quant
        .as_ref()
        .map(|q| format!("{} · {}", entry.format, q))
        .unwrap_or_else(|| entry.format.clone());
    let provider_label = if entry.provider.trim().is_empty() {
        "Unknown".to_string()
    } else {
        entry.provider.trim().to_string()
    };
    let description = if entry.recommended {
        format!(
            "{} · {} · {} (recommended)",
            provider_label, format_label, entry.engine
        )
    } else {
        format!("{} · {} · {}", provider_label, format_label, entry.engine)
    };

    ModelCatalogEntry {
        id: entry.id.clone(),
        name: entry.name.clone(),
        provider: ModelProvider::Gguf,
        provider_label,
        version: filename.clone().unwrap_or_else(|| entry.id.clone()),
        description,
        purpose: String::new(),
        recommended: entry.recommended,
        size_bytes: parse_size_bytes(entry.size.as_deref()),
        quant,
        capabilities: ModelCapabilities::gguf(),
        engine: entry.engine.clone(),
        format: entry.format.clone(),
        download_url: Some(entry.download_url.clone()),
        sha256: entry
            .sha256
            .as_ref()
            .filter(|s| !s.trim().is_empty())
            .cloned(),
        size_label: entry.size.clone(),
        repo: None,
        filename,
    }
}

pub fn filename_from_url(url: &str) -> Option<String> {
    let path = url.split('?').next().unwrap_or(url);
    path.rsplit('/').next().map(str::to_string)
}

fn infer_quant_from_url(url: &str) -> Option<String> {
    filename_from_url(url).and_then(|name| infer_quant(&name))
}

fn infer_quant(filename: &str) -> Option<String> {
    let upper = filename.to_ascii_uppercase();
    for token in ["Q8_0", "Q6_K", "Q5_K_M", "Q5_K_S", "Q4_K_M", "Q4_0", "Q3_K_M", "Q2_K"] {
        if upper.contains(token) {
            return Some(token.to_string());
        }
    }
    None
}

fn parse_size_bytes(size: Option<&str>) -> Option<u64> {
    let label = size?.trim().to_ascii_uppercase();
    let (num_str, unit) = label.split_at(
        label
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(label.len()),
    );
    let value: f64 = num_str.parse().ok()?;
    let multiplier = match unit.trim() {
        "GB" | "G" => 1024.0 * 1024.0 * 1024.0,
        "MB" | "M" => 1024.0 * 1024.0,
        "KB" | "K" => 1024.0,
        "" => 1.0,
        _ => return None,
    };
    Some((value * multiplier) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_v2_entry_to_catalog() {
        let entry = BuiltinRegistryEntry {
            id: "qwen3-8b-judge".into(),
            name: "Qwen3 8B".into(),
            provider: "Qwen Team".into(),
            purpose: String::new(),
            recommended: true,
            engine: "llama.cpp".into(),
            format: "gguf".into(),
            size: Some("4.7GB".into()),
            sha256: None,
            download_url: huggingface_url(
                "Qwen/Qwen3-8B-GGUF",
                "qwen3-8b-q4_k_m.gguf",
                Some("main"),
            ),
        };
        let catalog = entry_to_catalog(&entry);
        assert_eq!(catalog.id, "qwen3-8b-judge");
        assert_eq!(catalog.provider_label, "Qwen Team");
        assert_eq!(catalog.provider, ModelProvider::Gguf);
        assert_eq!(catalog.quant.as_deref(), Some("Q4_K_M"));
        assert!(catalog.download_url.is_some());
        assert!(catalog.description.contains("Qwen Team"));
    }

    #[test]
    fn loads_repo_registry() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/models.json");
        let catalog = BuiltinCatalog::load_from_path(&path).expect("load registry");
        assert!(catalog.meta().validation.valid >= 3);
        assert_eq!(catalog.meta().validation.invalid, 0);
    }
}
