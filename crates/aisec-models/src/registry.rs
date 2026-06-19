use std::collections::HashMap;
use std::path::{Path, PathBuf};

use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::{ModelError, ModelResult};
use crate::runtime::{infer_capabilities, infer_provider, infer_version};
use crate::types::{ModelEntry, ModelFormat, ModelProvider, ModelSource};

const REGISTRY_VERSION: u32 = 1;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct RegistrySnapshot {
    version: u32,
    entries: Vec<ModelEntry>,
}

/// In-memory model registry with vault-backed paths.
#[derive(Debug, Default)]
pub struct ModelRegistry {
    entries: HashMap<String, ModelEntry>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn registry_path(vault: &Path) -> PathBuf {
        vault.join("registry.json")
    }

    pub fn load_from_vault(vault: &Path) -> ModelResult<Self> {
        let path = Self::registry_path(vault);
        if !path.exists() {
            return Ok(Self::new());
        }
        let raw = std::fs::read_to_string(&path).map_err(ModelError::Io)?;
        let snapshot: RegistrySnapshot =
            serde_json::from_str(&raw).map_err(|e| ModelError::invalid(e.to_string()))?;
        let mut registry = Self::new();
        for entry in snapshot.entries {
            registry.entries.insert(entry.id.clone(), entry);
        }
        Ok(registry)
    }

    pub fn save_to_vault(&self, vault: &Path) -> ModelResult<()> {
        std::fs::create_dir_all(vault).map_err(ModelError::Io)?;
        let snapshot = RegistrySnapshot {
            version: REGISTRY_VERSION,
            entries: self.entries.values().cloned().collect(),
        };
        let json = serde_json::to_string_pretty(&snapshot)
            .map_err(|e| ModelError::invalid(e.to_string()))?;
        std::fs::write(Self::registry_path(vault), json).map_err(ModelError::Io)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn list(&self) -> Vec<&ModelEntry> {
        let mut items: Vec<_> = self.entries.values().collect();
        items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        items
    }

    pub fn get(&self, id: &str) -> Option<&ModelEntry> {
        self.entries.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut ModelEntry> {
        self.entries.get_mut(id)
    }

    pub fn register_local(
        &mut self,
        name: impl Into<String>,
        path: impl AsRef<Path>,
    ) -> ModelResult<ModelEntry> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            return Err(ModelError::invalid(format!(
                "model file not found: {}",
                path.display()
            )));
        }

        let format = ModelFormat::from_path(&path)
            .ok_or_else(|| ModelError::invalid("only .gguf models are supported"))?;

        let source = ModelSource::Local { path: path.clone() };
        let provider = infer_provider(&source);
        let size_bytes = std::fs::metadata(&path).ok().map(|m| m.len());
        let now = OffsetDateTime::now_utc();
        let id = Uuid::new_v4().to_string();

        let entry = ModelEntry {
            id: id.clone(),
            name: name.into(),
            format,
            provider,
            version: infer_version(&source),
            capabilities: infer_capabilities(provider),
            source,
            file_path: path,
            size_bytes,
            checksum_sha256: None,
            verified: false,
            created_at: now,
            updated_at: now,
            metadata: serde_json::json!({}),
        };

        self.entries.insert(id, entry.clone());
        Ok(entry)
    }

    pub fn register_ollama(
        &mut self,
        vault: &Path,
        name: impl Into<String>,
        model: impl Into<String>,
        base_url: impl Into<String>,
    ) -> ModelResult<ModelEntry> {
        let name = name.into();
        let model = model.into();
        let base_url = base_url.into();
        let id = Uuid::new_v4().to_string();
        let model_dir = Self::model_dir(vault, &id);
        std::fs::create_dir_all(&model_dir).map_err(ModelError::Io)?;
        let ref_path = model_dir.join("reference.json");
        let source = ModelSource::Ollama {
            model: model.clone(),
            base_url: base_url.clone(),
        };
        let sidecar = serde_json::json!({
            "model": model,
            "base_url": base_url,
        });
        std::fs::write(&ref_path, serde_json::to_string_pretty(&sidecar).unwrap())
            .map_err(ModelError::Io)?;

        let now = OffsetDateTime::now_utc();
        let provider = ModelProvider::Ollama;
        let entry = ModelEntry {
            id: id.clone(),
            name,
            format: ModelFormat::Gguf,
            provider,
            version: infer_version(&source),
            capabilities: infer_capabilities(provider),
            source,
            file_path: ref_path,
            size_bytes: None,
            checksum_sha256: None,
            verified: false,
            created_at: now,
            updated_at: now,
            metadata: serde_json::json!({ "runtime": "ollama" }),
        };

        self.entries.insert(id, entry.clone());
        Ok(entry)
    }

    pub fn register_entry(&mut self, entry: ModelEntry) -> ModelResult<()> {
        if self.entries.contains_key(&entry.id) {
            return Err(ModelError::invalid(format!(
                "model id already registered: {}",
                entry.id
            )));
        }
        self.entries.insert(entry.id.clone(), entry);
        Ok(())
    }

    pub fn upsert_entry(&mut self, entry: ModelEntry) {
        self.entries.insert(entry.id.clone(), entry);
    }

    /// Register or update a third-party cloud API model reference (no local GGUF file).
    pub fn register_remote(
        &mut self,
        provider: impl Into<String>,
        model: impl Into<String>,
        base_url: Option<String>,
        region: Option<String>,
    ) -> ModelEntry {
        let provider_str = provider.into();
        let model_str = model.into();
        let id = format!("remote-{provider_str}");
        let label = provider_label(&provider_str);
        let name = format!("{label} — {model_str}");
        let path = format!("remote://{provider_str}/{model_str}");
        let source = ModelSource::Remote {
            provider: provider_str.clone(),
            model: model_str.clone(),
            base_url,
            region,
        };
        let now = OffsetDateTime::now_utc();
        let entry = ModelEntry {
            id: id.clone(),
            name,
            format: ModelFormat::Api,
            provider: ModelProvider::Remote,
            version: model_str,
            capabilities: infer_capabilities(ModelProvider::Remote),
            source,
            file_path: PathBuf::from(path),
            size_bytes: None,
            checksum_sha256: None,
            verified: true,
            created_at: self
                .entries
                .get(&id)
                .map(|existing| existing.created_at)
                .unwrap_or(now),
            updated_at: now,
            metadata: serde_json::json!({ "remoteProvider": provider_str }),
        };
        self.upsert_entry(entry.clone());
        entry
    }

    pub fn update_verification(
        &mut self,
        id: &str,
        checksum_sha256: String,
        verified: bool,
    ) -> ModelResult<&ModelEntry> {
        let entry = self
            .entries
            .get_mut(id)
            .ok_or_else(|| ModelError::not_found(id))?;
        entry.checksum_sha256 = Some(checksum_sha256);
        entry.verified = verified;
        entry.updated_at = OffsetDateTime::now_utc();
        Ok(self.entries.get(id).expect("entry exists"))
    }

    pub fn remove(&mut self, id: &str) -> ModelResult<ModelEntry> {
        self.entries
            .remove(id)
            .ok_or_else(|| ModelError::not_found(id))
    }

    pub fn model_dir(vault: &Path, model_id: &str) -> PathBuf {
        vault.join("models").join(model_id)
    }

    pub fn model_file(vault: &Path, model_id: &str, filename: &str) -> PathBuf {
        Self::model_dir(vault, model_id).join(filename)
    }
}

fn provider_label(provider: &str) -> String {
    match provider {
        "openai" => "OpenAI",
        "anthropic" => "Anthropic",
        "gemini" | "google" => "Google",
        "azure" => "Azure",
        "bedrock" => "AWS Bedrock",
        other => other,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn register_local_gguf() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.gguf");
        std::fs::write(&path, b"gguf-stub").unwrap();

        let mut registry = ModelRegistry::new();
        let entry = registry.register_local("test-model", &path).unwrap();
        assert_eq!(entry.format, ModelFormat::Gguf);
        assert_eq!(entry.provider, ModelProvider::Gguf);
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn rejects_non_gguf() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("model.bin");
        std::fs::write(&path, b"x").unwrap();

        let mut registry = ModelRegistry::new();
        assert!(registry.register_local("bad", &path).is_err());
    }

    #[test]
    fn persists_registry() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        let path = dir.path().join("test.gguf");
        std::fs::write(&path, b"gguf-stub").unwrap();

        let mut registry = ModelRegistry::new();
        registry.register_local("test-model", &path).unwrap();
        registry.save_to_vault(&vault).unwrap();

        let loaded = ModelRegistry::load_from_vault(&vault).unwrap();
        assert_eq!(loaded.len(), 1);
    }
}
