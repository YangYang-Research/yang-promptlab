use std::path::{Path, PathBuf};

use promptlab_storage::{Database, ModelRepository, UpsertModelEntry};
use time::OffsetDateTime;
use tracing::info;

use crate::error::{ModelError, ModelResult};
use crate::hardware::detect_hardware;
use crate::registry::ModelRegistry;
use crate::types::{HardwareProfile, ModelEntry, ModelProvider, VerificationResult};
use crate::verify::VerificationEngine;

/// Top-level model manager orchestrating registry, verification, and remote providers.
pub struct LocalModelManager {
    vault_path: PathBuf,
    registry: ModelRegistry,
    /// When set, registry is persisted to SQLite (`models` table). Absent in unit tests.
    db: Option<Database>,
    hardware: HardwareProfile,
}

impl LocalModelManager {
    /// In-memory manager (no SQLite). Used by unit tests and helpers that do not need persistence.
    pub fn new(vault_path: impl AsRef<Path>) -> ModelResult<Self> {
        let vault_path = vault_path.as_ref().to_path_buf();
        std::fs::create_dir_all(&vault_path).map_err(ModelError::Io)?;

        let hardware = detect_hardware()?;
        Ok(Self {
            vault_path,
            registry: ModelRegistry::new(),
            db: None,
            hardware,
        })
    }

    /// Load registry from SQLite.
    pub async fn new_with_db(vault_path: impl AsRef<Path>, db: Database) -> ModelResult<Self> {
        let vault_path = vault_path.as_ref().to_path_buf();
        std::fs::create_dir_all(&vault_path).map_err(ModelError::Io)?;

        let hardware = detect_hardware()?;
        let registry = load_registry_from_db(&db).await?;

        Ok(Self {
            vault_path,
            registry,
            db: Some(db),
            hardware,
        })
    }

    pub fn vault_path(&self) -> &Path {
        &self.vault_path
    }

    pub fn registry(&self) -> &ModelRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut ModelRegistry {
        &mut self.registry
    }

    pub fn hardware(&self) -> &HardwareProfile {
        &self.hardware
    }

    async fn persist(&self) -> ModelResult<()> {
        let Some(db) = self.db.as_ref() else {
            return Ok(());
        };
        let entries = self
            .registry
            .list()
            .into_iter()
            .map(|entry| model_entry_to_upsert(entry))
            .collect::<ModelResult<Vec<_>>>()?;
        db.repositories()
            .models()
            .replace_all(entries)
            .await
            .map_err(ModelError::from)?;
        Ok(())
    }

    /// Register or update a third-party cloud model in the vault registry.
    pub async fn register_third_party(
        &mut self,
        provider: impl Into<String>,
        model: impl Into<String>,
        base_url: Option<String>,
        region: Option<String>,
    ) -> ModelResult<ModelEntry> {
        let entry = self.registry.register_remote(provider, model, base_url, region);
        self.persist().await?;
        Ok(entry)
    }

    /// Remove a model from the registry.
    pub async fn remove_model(&mut self, model_id: &str) -> ModelResult<ModelEntry> {
        let entry = self.registry.remove(model_id)?;
        self.persist().await?;
        info!(id = %model_id, "removed model");
        Ok(entry)
    }

    /// Verify a registered model (remote always valid).
    pub async fn verify_model(&mut self, model_id: &str) -> ModelResult<VerificationResult> {
        let entry = self
            .registry
            .get(model_id)
            .ok_or_else(|| ModelError::not_found(model_id))?
            .clone();

        match entry.provider {
            ModelProvider::Remote => Ok(VerificationResult {
                file_path: entry.file_path,
                expected_sha256: None,
                actual_sha256: "remote-api".into(),
                size_bytes: 0,
                valid: true,
            }),
        }
    }

    /// Verify an arbitrary file on disk.
    pub async fn verify_file(
        &self,
        path: &Path,
        expected_sha256: Option<&str>,
    ) -> ModelResult<VerificationResult> {
        VerificationEngine::verify_file(path, expected_sha256).await
    }

    /// Local inference is unsupported — use the remote third-party gateway.
    pub async fn inference_engine(
        &self,
        _model_id: &str,
    ) -> ModelResult<crate::runtime::LocalInferenceEngine> {
        Err(ModelError::runtime("use a remote third-party provider"))
    }

    /// Local completion smoke tests are unsupported.
    pub async fn test_inference(&self, _model_id: &str) -> ModelResult<String> {
        Err(ModelError::runtime("use a remote third-party provider"))
    }

    /// Local chat smoke tests are unsupported.
    pub async fn test_chat(&self, _model_id: &str) -> ModelResult<String> {
        Err(ModelError::runtime("use a remote third-party provider"))
    }

    pub fn list_models(&self) -> Vec<&ModelEntry> {
        self.registry.list()
    }

    /// Aggregate vault sizes for desktop UI cards (remote only).
    pub fn vault_stats(&self) -> ModelResult<crate::types::VaultStats> {
        let models = self.list_models();
        let installed_bytes = models.iter().filter_map(|entry| entry.size_bytes).sum();
        Ok(crate::types::VaultStats {
            registered_count: models.len(),
            installed_local_count: 0,
            installed_bytes,
            vault_path: self.vault_path.clone(),
        })
    }

    pub async fn update_model_metadata(
        &mut self,
        model_id: &str,
        metadata: serde_json::Value,
    ) -> ModelResult<&ModelEntry> {
        let entry = self
            .registry
            .get_mut(model_id)
            .ok_or_else(|| ModelError::not_found(model_id))?;
        entry.metadata = metadata;
        entry.updated_at = OffsetDateTime::now_utc();
        self.persist().await?;
        Ok(self.registry.get(model_id).expect("entry exists"))
    }

    pub async fn set_model_verified(
        &mut self,
        model_id: &str,
        verified: bool,
    ) -> ModelResult<&ModelEntry> {
        let entry = self
            .registry
            .get_mut(model_id)
            .ok_or_else(|| ModelError::not_found(model_id))?;
        entry.verified = verified;
        entry.updated_at = OffsetDateTime::now_utc();
        self.persist().await?;
        Ok(self.registry.get(model_id).expect("entry exists"))
    }

    pub fn get_model(&self, model_id: &str) -> Option<&ModelEntry> {
        self.registry.get(model_id)
    }
}

fn model_entry_to_upsert(entry: &ModelEntry) -> ModelResult<UpsertModelEntry> {
    let entry_json =
        serde_json::to_string(entry).map_err(|e| ModelError::invalid(e.to_string()))?;
    let metadata_json = if entry.metadata.is_null() {
        None
    } else {
        Some(
            serde_json::to_string(&entry.metadata)
                .map_err(|e| ModelError::invalid(e.to_string()))?,
        )
    };
    Ok(UpsertModelEntry {
        id: entry.id.clone(),
        name: entry.name.clone(),
        provider: entry.provider.as_str().to_string(),
        format: entry.format.as_str().to_string(),
        file_path: entry.file_path.to_string_lossy().into_owned(),
        checksum_sha256: entry.checksum_sha256.clone(),
        size_bytes: entry.size_bytes.map(|n| n as i64),
        verified: entry.verified,
        entry_json,
        metadata_json,
        created_at: entry.created_at,
        updated_at: entry.updated_at,
    })
}

async fn load_registry_from_db(db: &Database) -> ModelResult<ModelRegistry> {
    let repo = db.repositories().models();
    let count = repo.count().await.map_err(ModelError::from)?;
    if count == 0 {
        return Ok(ModelRegistry::new());
    }

    let rows = repo.list().await.map_err(ModelError::from)?;
    let mut entries = Vec::with_capacity(rows.len());
    for row in rows {
        match serde_json::from_str::<ModelEntry>(&row.entry_json) {
            Ok(entry) => entries.push(entry),
            Err(err) => {
                tracing::warn!(
                    id = %row.id,
                    error = %err,
                    "skipping unreadable model row"
                );
            }
        }
    }
    Ok(ModelRegistry::from_entries(entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn register_third_party_and_list() {
        let dir = tempfile::tempdir().unwrap();
        let mut mgr = LocalModelManager::new(dir.path().join("vault")).unwrap();
        let entry = mgr
            .register_third_party("openai", "gpt-4o", None, None)
            .await
            .unwrap();
        assert_eq!(entry.provider, ModelProvider::Remote);
        assert_eq!(mgr.list_models().len(), 1);
        assert!(mgr.hardware().cpu_cores >= 1);
    }
}
