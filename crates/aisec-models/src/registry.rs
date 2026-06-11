use std::collections::HashMap;
use std::path::{Path, PathBuf};

use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::{ModelError, ModelResult};
use crate::types::{ModelEntry, ModelFormat, ModelSource};

/// In-memory model registry with vault-backed paths.
#[derive(Debug, Default)]
pub struct ModelRegistry {
    entries: HashMap<String, ModelEntry>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self::default()
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

        let size_bytes = std::fs::metadata(&path).ok().map(|m| m.len());
        let now = OffsetDateTime::now_utc();
        let id = Uuid::new_v4().to_string();

        let entry = ModelEntry {
            id: id.clone(),
            name: name.into(),
            format,
            source: ModelSource::Local { path: path.clone() },
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
}
