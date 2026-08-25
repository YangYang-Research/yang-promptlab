use std::collections::HashMap;
use std::path::{Path, PathBuf};

use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::{ModelError, ModelResult};
use crate::runtime::{infer_capabilities, infer_version};
use crate::types::{ModelEntry, ModelFormat, ModelProvider, ModelSource};

const REGISTRY_VERSION: u32 = 1;
pub const MODEL_REGISTRY_DIR: &str = "model-registry";
const LEGACY_MODEL_STORAGE_DIR: &str = "models";

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

    pub fn from_entries(entries: impl IntoIterator<Item = ModelEntry>) -> Self {
        let mut registry = Self::new();
        for entry in entries {
            registry.entries.insert(entry.id.clone(), entry);
        }
        registry
    }

    pub fn registry_path(vault: &Path) -> PathBuf {
        vault.join("registry.json")
    }

    pub fn migrated_registry_path(vault: &Path) -> PathBuf {
        vault.join("registry.json.migrated")
    }

    pub fn load_from_vault(vault: &Path) -> ModelResult<Self> {
        let path = Self::registry_path(vault);
        if !path.exists() {
            return Ok(Self::new());
        }
        let raw = std::fs::read_to_string(&path).map_err(ModelError::Io)?;
        if raw.trim().is_empty() {
            tracing::warn!(path = %path.display(), "model registry file is empty — starting fresh");
            return Ok(Self::new());
        }
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
        let id = remote_entry_id(&provider_str, &model_str);
        let path = format!("remote://{provider_str}/{model_str}");
        let source = ModelSource::Remote {
            provider: provider_str.clone(),
            model: model_str.clone(),
            base_url,
            region,
        };
        let now = OffsetDateTime::now_utc();
        let previous = self.entries.get(&id);
        let created_at = previous.map(|existing| existing.created_at).unwrap_or(now);
        let verified = previous.map(|existing| existing.verified).unwrap_or(false);
        let mut metadata = previous
            .map(|existing| existing.metadata.clone())
            .unwrap_or_else(|| serde_json::json!({}));
        if let serde_json::Value::Object(ref mut map) = metadata {
            map.insert(
                "remoteProvider".to_string(),
                serde_json::Value::String(provider_str.clone()),
            );
        }
        let entry = ModelEntry {
            id: id.clone(),
            name: model_str.clone(),
            format: ModelFormat::Api,
            provider: ModelProvider::Remote,
            version: model_str,
            capabilities: infer_capabilities(ModelProvider::Remote),
            source,
            file_path: PathBuf::from(path),
            size_bytes: None,
            checksum_sha256: None,
            verified,
            created_at,
            updated_at: now,
            metadata,
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

    pub fn model_storage_root(vault: &Path) -> PathBuf {
        vault.join(MODEL_REGISTRY_DIR)
    }

    pub fn model_dir(vault: &Path, model_id: &str) -> PathBuf {
        Self::model_storage_root(vault).join(model_id)
    }

    pub fn model_file(vault: &Path, model_id: &str, filename: &str) -> PathBuf {
        Self::model_dir(vault, model_id).join(filename)
    }

    /// User-facing URI for a model file under the app data vault (`app://models/...`).
    pub fn display_uri(vault: &Path, file_path: &Path) -> String {
        let raw = file_path.to_string_lossy();
        if raw.starts_with("remote://") {
            return raw.into_owned();
        }
        if let Ok(rel) = file_path.strip_prefix(vault) {
            let rel = rel
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches('/')
                .to_string();
            return format!("app://models/{rel}");
        }
        raw.into_owned()
    }

    /// Move `vault/models/{id}` → `vault/model-registry/{id}` and rewrite registry paths.
    pub fn migrate_storage_layout(vault: &Path, registry: &mut Self) -> ModelResult<bool> {
        let legacy = vault.join(LEGACY_MODEL_STORAGE_DIR);
        let current = Self::model_storage_root(vault);
        let mut changed = false;

        if legacy.is_dir() {
            std::fs::create_dir_all(&current).map_err(ModelError::Io)?;
            for entry in std::fs::read_dir(&legacy).map_err(ModelError::Io)? {
                let entry = entry.map_err(ModelError::Io)?;
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let dest = current.join(entry.file_name());
                if dest.exists() {
                    continue;
                }
                std::fs::rename(&path, &dest).map_err(ModelError::Io)?;
                changed = true;
            }
            if std::fs::read_dir(&legacy)
                .map(|mut dir| dir.next().is_none())
                .unwrap_or(false)
            {
                let _ = std::fs::remove_dir(&legacy);
            }
        }

        for entry in registry.entries.values_mut() {
            if let Ok(rel) = entry.file_path.strip_prefix(&legacy) {
                entry.file_path = current.join(rel);
                changed = true;
            }
        }

        Ok(changed)
    }
}

/// Stable registry id for a third-party provider + model pair.
pub fn remote_entry_id(provider: &str, model: &str) -> String {
    let provider_part = slug_identifier(provider);
    let model_part = slug_identifier(model);
    if model_part.is_empty() {
        format!("remote-{provider_part}")
    } else {
        format!("remote-{provider_part}-{model_part}")
    }
}

fn slug_identifier(value: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in value.trim().to_ascii_lowercase().chars() {
        let normalized = if ch.is_ascii_alphanumeric() { ch } else { '-' };
        if normalized == '-' {
            if prev_dash || out.is_empty() {
                continue;
            }
            prev_dash = true;
        } else {
            prev_dash = false;
        }
        out.push(normalized);
    }
    while out.ends_with('-') {
        out.pop();
    }
    const MAX_LEN: usize = 96;
    if out.len() > MAX_LEN {
        out.truncate(MAX_LEN);
        while out.ends_with('-') {
            out.pop();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn model_dir_uses_model_registry() {
        let vault = Path::new("/data/models");
        assert_eq!(
            ModelRegistry::model_dir(vault, "abc"),
            Path::new("/data/models/model-registry/abc")
        );
    }

    #[test]
    fn display_uri_formats_app_scheme() {
        let vault = Path::new("/data/app/models");
        let file = vault.join("model-registry/abc/model.gguf");
        assert_eq!(
            ModelRegistry::display_uri(vault, &file),
            "app://models/model-registry/abc/model.gguf"
        );
    }

    #[test]
    fn migrates_legacy_model_storage() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        std::fs::create_dir_all(vault.join("models/model-a")).unwrap();
        std::fs::write(vault.join("models/model-a/weights.gguf"), b"gguf").unwrap();

        let mut registry = ModelRegistry::new();
        registry
            .upsert_entry(ModelEntry {
                id: "model-a".into(),
                name: "model-a".into(),
                format: ModelFormat::Gguf,
                provider: ModelProvider::Gguf,
                version: "1".into(),
                capabilities: infer_capabilities(ModelProvider::Gguf),
                source: ModelSource::Local {
                    path: vault.join("models/model-a/weights.gguf"),
                },
                file_path: vault.join("models/model-a/weights.gguf"),
                size_bytes: Some(4),
                checksum_sha256: None,
                verified: true,
                created_at: OffsetDateTime::now_utc(),
                updated_at: OffsetDateTime::now_utc(),
                metadata: serde_json::json!({}),
            });

        let changed = ModelRegistry::migrate_storage_layout(&vault, &mut registry).unwrap();
        assert!(changed);
        assert!(vault.join("model-registry/model-a/weights.gguf").is_file());
        assert_eq!(
            registry.get("model-a").unwrap().file_path,
            vault.join("model-registry/model-a/weights.gguf")
        );
    }

    #[test]
    fn persists_registry() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");

        let mut registry = ModelRegistry::new();
        registry.register_remote("openai", "gpt-4o", None, None);
        registry.save_to_vault(&vault).unwrap();

        let loaded = ModelRegistry::load_from_vault(&vault).unwrap();
        assert_eq!(loaded.len(), 1);
    }

    #[test]
    fn register_multiple_remote_same_provider() {
        let mut registry = ModelRegistry::new();
        let a = registry.register_remote("openai", "gpt-4o", None, None);
        let b = registry.register_remote("openai", "gpt-4o-mini", None, None);
        assert_ne!(a.id, b.id);
        assert_eq!(a.id, "remote-openai-gpt-4o");
        assert_eq!(b.id, "remote-openai-gpt-4o-mini");
        assert_eq!(registry.len(), 2);
        assert!(!a.verified);
        assert!(!b.verified);
    }

    #[test]
    fn register_remote_preserves_metadata_on_update() {
        let mut registry = ModelRegistry::new();
        let mut entry = registry.register_remote("openai", "gpt-4o", None, None);
        entry.metadata = serde_json::json!({ "apiKeyEnv": "OPENAI_API_KEY" });
        registry.upsert_entry(entry);

        let updated = registry.register_remote("openai", "gpt-4o", Some("https://api.openai.com/v1".into()), None);
        assert_eq!(updated.id, "remote-openai-gpt-4o");
        assert_eq!(
            updated.metadata.get("apiKeyEnv").and_then(|v| v.as_str()),
            Some("OPENAI_API_KEY")
        );
    }
}
