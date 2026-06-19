use std::path::{Path, PathBuf};

use time::OffsetDateTime;
use tracing::{info, instrument, warn};
use uuid::Uuid;

use crate::builtin_catalog::BuiltinCatalog;
use crate::catalog::find_catalog_entry;
use crate::download::{DownloadCoordinator, DownloadManager, DownloadOptions};
use crate::import_pack::{extract_gguf_from_zip, validate_gguf_path};
use crate::error::{ModelError, ModelResult};
use crate::hardware::detect_hardware;
use crate::registry::ModelRegistry;
use crate::runtime::{
    infer_capabilities, infer_provider, infer_version, InferenceRuntime, LocalInferenceEngine,
    LlamaCppConfig, LlamaCppRuntime,
};
use crate::types::{
    ChatMessage, ChatRequest, DownloadProgress, DownloadStatus, HardwareProfile, HuggingFaceDownloadRequest,
    InferenceRequest, ModelCatalogEntry, ModelEntry, ModelFormat, ModelProvider, ModelSource,
    VerificationResult,
};
use crate::builtin_catalog::BuiltinCatalogMeta;
use crate::verify::VerificationEngine;

/// Top-level local model manager orchestrating registry, downloads, verification, and runtime.
pub struct LocalModelManager {
    vault_path: PathBuf,
    registry: ModelRegistry,
    downloader: DownloadManager,
    download_coordinator: DownloadCoordinator,
    catalog: Vec<ModelCatalogEntry>,
    catalog_meta: BuiltinCatalogMeta,
    hardware: HardwareProfile,
    runtime: LlamaCppRuntime,
}

impl LocalModelManager {
    pub fn new(vault_path: impl AsRef<Path>) -> ModelResult<Self> {
        let vault_path = vault_path.as_ref().to_path_buf();
        std::fs::create_dir_all(&vault_path).map_err(ModelError::Io)?;

        let hardware = detect_hardware()?;
        let mut llama_config = LlamaCppConfig::default();
        llama_config.n_gpu_layers = hardware.recommended_gpu_layers();
        let registry = ModelRegistry::load_from_vault(&vault_path)?;

        Ok(Self {
            vault_path,
            registry,
            downloader: DownloadManager::with_defaults(),
            download_coordinator: DownloadCoordinator::new(DownloadManager::with_defaults()),
            catalog: Vec::new(),
            catalog_meta: BuiltinCatalogMeta::default(),
            hardware,
            runtime: LlamaCppRuntime::new(llama_config),
        })
    }

    pub fn with_catalog(mut self, catalog: BuiltinCatalog) -> Self {
        self.catalog_meta = catalog.meta().clone();
        self.catalog = catalog.entries().to_vec();
        self
    }

    /// Point the vault inference runtime at a bundled or system `llama-server` binary.
    pub fn with_llama_binary(mut self, binary: impl AsRef<Path>) -> Self {
        let mut config = self.llama_config();
        config.binary_path = binary.as_ref().to_path_buf();
        self.runtime = LlamaCppRuntime::new(config);
        self
    }

    pub fn with_download_options(
        vault_path: impl AsRef<Path>,
        download_options: DownloadOptions,
    ) -> ModelResult<Self> {
        let mut mgr = Self::new(vault_path)?;
        mgr.downloader = DownloadManager::new(download_options.clone());
        mgr.download_coordinator = DownloadCoordinator::new(DownloadManager::new(download_options));
        Ok(mgr)
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

    pub fn runtime(&self) -> &LlamaCppRuntime {
        &self.runtime
    }

    pub fn runtime_mut(&mut self) -> &mut LlamaCppRuntime {
        &mut self.runtime
    }

    pub fn llama_config(&self) -> LlamaCppConfig {
        let mut config = LlamaCppConfig::default();
        config.n_gpu_layers = self.hardware.recommended_gpu_layers();
        config
    }

    fn persist(&self) -> ModelResult<()> {
        self.registry.save_to_vault(&self.vault_path)
    }

    pub fn catalog_meta(&self) -> &BuiltinCatalogMeta {
        &self.catalog_meta
    }

    pub fn download_coordinator(&self) -> &DownloadCoordinator {
        &self.download_coordinator
    }

    /// Built-in registry catalog for browse UI.
    pub fn browse_catalog(&self) -> &[ModelCatalogEntry] {
        &self.catalog
    }

    pub fn find_catalog_entry(&self, catalog_id: &str) -> Option<&ModelCatalogEntry> {
        find_catalog_entry(&self.catalog, catalog_id)
    }

    /// Import an existing local GGUF file into the vault registry.
    pub fn import_local(&mut self, name: impl Into<String>, path: impl AsRef<Path>) -> ModelResult<ModelEntry> {
        let path = path.as_ref();
        validate_gguf_path(path)?;
        let entry = self.registry.register_local(name, path)?;
        self.persist()?;
        info!(id = %entry.id, path = %path.display(), "imported local model");
        Ok(entry)
    }

    pub fn import_zip_package(
        &mut self,
        name: impl Into<String>,
        zip_path: impl AsRef<Path>,
    ) -> ModelResult<ModelEntry> {
        let zip_path = zip_path.as_ref();
        let model_id = Uuid::new_v4().to_string();
        let model_dir = ModelRegistry::model_dir(&self.vault_path, &model_id);
        let extracted = extract_gguf_from_zip(zip_path, &model_dir)?;
        let entry = self.registry.register_local(name, &extracted)?;
        self.persist()?;
        info!(id = %entry.id, zip = %zip_path.display(), "imported zip model package");
        Ok(entry)
    }

    /// Install from catalog entry id (GGUF download from registry URL).
    #[instrument(skip(self))]
    pub async fn install_catalog(
        &mut self,
        catalog_id: &str,
        _ollama_base_url: Option<String>,
    ) -> ModelResult<ModelEntry> {
        let _ = _ollama_base_url;
        self.download_catalog_entry(catalog_id).await
    }

    async fn download_catalog_entry(&mut self, catalog_id: &str) -> ModelResult<ModelEntry> {
        let catalog = self
            .find_catalog_entry(catalog_id)
            .ok_or_else(|| ModelError::not_found(format!("catalog entry: {catalog_id}")))?
            .clone();

        let url = catalog
            .download_url
            .as_deref()
            .filter(|u| !u.is_empty())
            .ok_or_else(|| ModelError::invalid("catalog entry missing download_url"))?;

        let filename = catalog
            .filename
            .clone()
            .or_else(|| crate::builtin_catalog::filename_from_url(url))
            .ok_or_else(|| ModelError::invalid("could not infer filename from download_url"))?;

        if !filename.to_lowercase().ends_with(".gguf") {
            return Err(ModelError::invalid("registry download must target a .gguf file"));
        }

        let model_id = Uuid::new_v4().to_string();
        let model_dir = ModelRegistry::model_dir(&self.vault_path, &model_id);
        tokio::fs::create_dir_all(&model_dir).await.map_err(ModelError::Io)?;
        let destination = model_dir.join(&filename);

        info!(url = %url, file = %filename, "starting registry GGUF download");

        let mut progress = self
            .downloader
            .download(url, &destination)
            .await?;
        progress.model_id = model_id.clone();
        progress.status = DownloadStatus::Completed;

        let expected_sha256 = catalog.sha256.as_deref().filter(|s| !s.is_empty());
        if expected_sha256.is_none() {
            warn!(
                catalog_id = %catalog_id,
                "registry entry has no sha256; installing without integrity verification"
            );
        }
        let verification = self.verify_file(&destination, expected_sha256).await?;
        if !verification.valid {
            return Err(ModelError::verification("post-download checksum mismatch"));
        }

        let source = ModelSource::Local {
            path: destination.clone(),
        };
        let provider = ModelProvider::Gguf;
        let now = OffsetDateTime::now_utc();
        let entry = ModelEntry {
            id: model_id,
            name: catalog.name,
            format: ModelFormat::Gguf,
            provider,
            version: filename,
            capabilities: infer_capabilities(provider),
            source,
            file_path: destination,
            size_bytes: Some(verification.size_bytes),
            checksum_sha256: Some(verification.actual_sha256),
            verified: true,
            created_at: now,
            updated_at: now,
            metadata: serde_json::json!({ "download": progress, "registry_id": catalog_id }),
        };

        self.registry.register_entry(entry.clone())?;
        self.persist()?;
        Ok(entry)
    }

    /// Start a background GGUF download for a registry catalog entry.
    pub async fn start_catalog_download(
        &mut self,
        catalog_id: &str,
    ) -> ModelResult<DownloadProgress> {
        let catalog = self
            .find_catalog_entry(catalog_id)
            .ok_or_else(|| ModelError::not_found(format!("catalog entry: {catalog_id}")))?
            .clone();

        let url = catalog
            .download_url
            .as_deref()
            .filter(|u| !u.is_empty())
            .ok_or_else(|| ModelError::invalid("catalog entry missing download_url"))?;

        let filename = catalog
            .filename
            .clone()
            .or_else(|| crate::builtin_catalog::filename_from_url(url))
            .ok_or_else(|| ModelError::invalid("could not infer filename from download_url"))?;

        let model_id = Uuid::new_v4().to_string();
        let model_dir = ModelRegistry::model_dir(&self.vault_path, &model_id);
        tokio::fs::create_dir_all(&model_dir).await.map_err(ModelError::Io)?;
        let destination = model_dir.join(&filename);

        self.download_coordinator
            .start_url_download(
                catalog_id,
                url,
                destination,
                catalog.sha256.clone(),
                catalog.size_bytes,
            )
            .await
    }

    pub async fn download_status(&self) -> Option<DownloadProgress> {
        self.download_coordinator.status().await
    }

    pub async fn pause_download(&self) -> ModelResult<DownloadProgress> {
        self.download_coordinator.pause().await
    }

    pub async fn resume_download(&self) -> ModelResult<DownloadProgress> {
        self.download_coordinator.resume().await
    }

    pub async fn cancel_download(&self) -> ModelResult<()> {
        self.download_coordinator.cancel().await
    }

    /// Finalize a completed background download into the vault registry.
    pub async fn finalize_active_download(&mut self) -> ModelResult<Option<ModelEntry>> {
        let Some((catalog_id, destination, progress)) =
            self.download_coordinator.take_if_completed().await
        else {
            return Ok(None);
        };

        let catalog = self
            .find_catalog_entry(&catalog_id)
            .ok_or_else(|| ModelError::not_found(format!("catalog entry: {catalog_id}")))?
            .clone();

        let expected_sha256 = catalog.sha256.as_deref().filter(|s| !s.is_empty());
        if expected_sha256.is_none() {
            warn!(
                catalog_id = %catalog_id,
                "registry entry has no sha256; installing without integrity verification"
            );
        }
        let verification = self.verify_file(&destination, expected_sha256).await?;
        if !verification.valid {
            return Err(ModelError::verification("post-download checksum mismatch"));
        }

        let source = ModelSource::Local {
            path: destination.clone(),
        };
        let provider = ModelProvider::Gguf;
        let now = OffsetDateTime::now_utc();
        let entry = ModelEntry {
            id: Uuid::new_v4().to_string(),
            name: catalog.name,
            format: ModelFormat::Gguf,
            provider,
            version: catalog.version.clone(),
            capabilities: infer_capabilities(provider),
            source,
            file_path: destination,
            size_bytes: Some(verification.size_bytes),
            checksum_sha256: Some(verification.actual_sha256),
            verified: true,
            created_at: now,
            updated_at: now,
            metadata: serde_json::json!({ "download": progress }),
        };

        self.registry.register_entry(entry.clone())?;
        self.persist()?;
        Ok(Some(entry))
    }
    #[instrument(skip(self, request))]
    pub async fn download_huggingface(
        &mut self,
        request: HuggingFaceDownloadRequest,
    ) -> ModelResult<ModelEntry> {
        if !request.filename.to_lowercase().ends_with(".gguf") {
            return Err(ModelError::invalid("HuggingFace filename must be .gguf"));
        }

        let model_id = Uuid::new_v4().to_string();
        let model_dir = ModelRegistry::model_dir(&self.vault_path, &model_id);
        tokio::fs::create_dir_all(&model_dir).await.map_err(ModelError::Io)?;

        let destination = model_dir.join(&request.filename);

        info!(repo = %request.repo, file = %request.filename, "starting HuggingFace download");

        let mut progress = self
            .downloader
            .download_huggingface(
                &request.repo,
                &request.filename,
                &destination,
                request.revision.as_deref(),
            )
            .await?;
        progress.model_id = model_id.clone();
        progress.status = DownloadStatus::Completed;

        let verification = self.verify_file(&destination, request.expected_sha256.as_deref()).await?;
        if !verification.valid {
            return Err(ModelError::verification("post-download checksum mismatch"));
        }

        let source = ModelSource::HuggingFace {
            repo: request.repo,
            filename: request.filename.clone(),
            revision: request.revision,
        };
        let provider = infer_provider(&source);
        let now = OffsetDateTime::now_utc();
        let entry = ModelEntry {
            id: model_id.clone(),
            name: request.name,
            format: ModelFormat::Gguf,
            provider,
            version: infer_version(&source),
            capabilities: infer_capabilities(provider),
            source,
            file_path: destination,
            size_bytes: Some(verification.size_bytes),
            checksum_sha256: Some(verification.actual_sha256),
            verified: true,
            created_at: now,
            updated_at: now,
            metadata: serde_json::json!({
                "download": progress,
                "hardware": {
                    "gpu_layers": self.hardware.recommended_gpu_layers(),
                }
            }),
        };

        self.registry.register_entry(entry.clone())?;
        self.persist()?;
        info!(id = %entry.id, "model registered after download");
        Ok(entry)
    }

    /// Register or update a third-party cloud model in the vault registry.
    pub fn register_third_party(
        &mut self,
        provider: impl Into<String>,
        model: impl Into<String>,
        base_url: Option<String>,
        region: Option<String>,
    ) -> ModelResult<ModelEntry> {
        let entry = self.registry.register_remote(provider, model, base_url, region);
        self.persist()?;
        Ok(entry)
    }

    /// Remove a model from the registry and delete vault files.
    pub async fn remove_model(&mut self, model_id: &str) -> ModelResult<ModelEntry> {
        let entry = self.registry.remove(model_id)?;
        let model_dir = ModelRegistry::model_dir(&self.vault_path, model_id);
        if model_dir.exists() {
            tokio::fs::remove_dir_all(&model_dir)
                .await
                .map_err(ModelError::Io)?;
        }
        self.persist()?;
        info!(id = %model_id, "removed model");
        Ok(entry)
    }

    /// Verify a registered model (SHA256 for GGUF, Ollama health for Ollama refs).
    pub async fn verify_model(&mut self, model_id: &str) -> ModelResult<VerificationResult> {
        let entry = self
            .registry
            .get(model_id)
            .ok_or_else(|| ModelError::not_found(model_id))?
            .clone();

        if entry.provider == ModelProvider::Remote {
            return Ok(VerificationResult {
                file_path: entry.file_path,
                expected_sha256: None,
                actual_sha256: "remote-api".into(),
                size_bytes: 0,
                valid: true,
            });
        }

        if entry.provider == ModelProvider::Ollama {
            let engine = LocalInferenceEngine::from_entry(entry.clone(), self.llama_config()).await?;
            let ok = engine.health().await.unwrap_or(false);
            if ok {
                self.registry.update_verification(model_id, "ollama-ok".into(), true)?;
                self.persist()?;
            }
            return Ok(VerificationResult {
                file_path: entry.file_path,
                expected_sha256: None,
                actual_sha256: if ok {
                    "ollama-ok".into()
                } else {
                    "ollama-unreachable".into()
                },
                size_bytes: 0,
                valid: ok,
            });
        }

        let expected = entry.checksum_sha256.as_deref();
        let result = self.verify_file(&entry.file_path, expected).await?;

        if result.valid {
            self.registry
                .update_verification(model_id, result.actual_sha256.clone(), true)?;
            self.persist()?;
        }

        Ok(result)
    }

    /// Verify an arbitrary file on disk.
    pub async fn verify_file(
        &self,
        path: &Path,
        expected_sha256: Option<&str>,
    ) -> ModelResult<VerificationResult> {
        VerificationEngine::verify_file(path, expected_sha256).await
    }

    /// Build a unified inference engine for a registered model.
    pub async fn inference_engine(&self, model_id: &str) -> ModelResult<LocalInferenceEngine> {
        let entry = self
            .registry
            .get(model_id)
            .ok_or_else(|| ModelError::not_found(model_id))?
            .clone();
        LocalInferenceEngine::from_entry(entry, self.llama_config()).await
    }

    /// Run a completion smoke test on a registered model.
    pub async fn test_inference(&self, model_id: &str) -> ModelResult<String> {
        let engine = self.inference_engine(model_id).await?;
        let response = engine
            .complete(InferenceRequest {
                prompt: "Reply with exactly: AISec OK".into(),
                max_tokens: 16,
                temperature: 0.0,
            })
            .await?;
        Ok(response.text)
    }

    /// Run a chat smoke test on a registered model.
    pub async fn test_chat(&self, model_id: &str) -> ModelResult<String> {
        let engine = self.inference_engine(model_id).await?;
        let response = engine
            .chat(ChatRequest {
                messages: vec![ChatMessage {
                    role: "user".into(),
                    content: "Reply with exactly: AISec OK".into(),
                }],
                max_tokens: 16,
                temperature: 0.0,
            })
            .await?;
        Ok(response.message.content)
    }

    /// Load a registered model into the llama.cpp runtime.
    pub async fn load(&mut self, model_id: &str) -> ModelResult<()> {
        let path = self
            .registry
            .get(model_id)
            .ok_or_else(|| ModelError::not_found(model_id))?
            .file_path
            .clone();

        if !path.exists() {
            return Err(ModelError::invalid(format!(
                "model file missing: {}",
                path.display()
            )));
        }

        self.runtime.load_model(&path).await
    }

    /// Unload the active llama.cpp model.
    pub async fn unload(&mut self) -> ModelResult<()> {
        self.runtime.unload().await
    }

    /// Run inference on the loaded model.
    pub async fn complete(
        &self,
        request: InferenceRequest,
    ) -> ModelResult<crate::types::InferenceResponse> {
        self.runtime.complete(request).await
    }

    pub fn list_models(&self) -> Vec<&ModelEntry> {
        self.registry.list()
    }

    /// Aggregate installed model sizes and on-disk vault usage.
    pub fn vault_stats(&self) -> ModelResult<crate::types::VaultStats> {
        let models = self.list_models();
        let installed_bytes = models
            .iter()
            .filter_map(|entry| entry.size_bytes)
            .sum();
        Ok(crate::types::VaultStats {
            model_count: models.len(),
            installed_bytes,
            disk_usage_bytes: dir_size(&self.vault_path)?,
            vault_path: self.vault_path.clone(),
        })
    }

    pub fn get_model(&self, model_id: &str) -> Option<&ModelEntry> {
        self.registry.get(model_id)
    }
}

fn dir_size(path: &Path) -> ModelResult<u64> {
    let metadata = std::fs::metadata(path).map_err(ModelError::Io)?;
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    if !metadata.is_dir() {
        return Ok(0);
    }

    let mut total = 0u64;
    for entry in std::fs::read_dir(path).map_err(ModelError::Io)? {
        let entry = entry.map_err(ModelError::Io)?;
        total = total.saturating_add(dir_size(&entry.path())?);
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::InferenceRuntime;

    #[tokio::test]
    async fn import_and_verify_local() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("vault");
        let model_path = dir.path().join("tiny.gguf");
        tokio::fs::write(&model_path, b"gguf-content").await.unwrap();

        let mut mgr = LocalModelManager::new(&vault).unwrap();
        let entry = mgr.import_local("tiny", &model_path).unwrap();

        let result = mgr.verify_model(&entry.id).await.unwrap();
        assert!(result.valid);
    }

    #[tokio::test]
    async fn mock_runtime_via_manager_types() {
        let dir = tempfile::tempdir().unwrap();
        let mut mgr = LocalModelManager::new(dir.path().join("vault")).unwrap();
        assert!(mgr.hardware().cpu_cores >= 1);
    }

    #[test]
    fn browse_uses_loaded_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let catalog_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/models.json");
        let catalog = BuiltinCatalog::load_from_path(&catalog_path).unwrap();
        let mgr = LocalModelManager::new(dir.path().join("vault"))
            .unwrap()
            .with_catalog(catalog);
        assert!(!mgr.browse_catalog().is_empty());
    }
}
