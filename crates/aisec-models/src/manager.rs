use std::path::{Path, PathBuf};

use time::OffsetDateTime;
use tracing::{info, instrument};
use uuid::Uuid;

use crate::catalog::{curated_catalog, find_catalog_entry};
use crate::download::{DownloadManager, DownloadOptions};
use crate::error::{ModelError, ModelResult};
use crate::hardware::detect_hardware;
use crate::registry::ModelRegistry;
use crate::runtime::{
    infer_capabilities, infer_provider, infer_version, InferenceRuntime, LocalInferenceEngine,
    LlamaCppConfig, LlamaCppRuntime, OllamaConfig, OllamaRuntime,
};
use crate::types::{
    ChatMessage, ChatRequest, DownloadStatus, HardwareProfile, HuggingFaceDownloadRequest,
    InferenceRequest, ModelCatalogEntry, ModelEntry, ModelFormat, ModelProvider, ModelSource,
    VerificationResult,
};
use crate::verify::VerificationEngine;

/// Top-level local model manager orchestrating registry, downloads, verification, and runtime.
pub struct LocalModelManager {
    vault_path: PathBuf,
    registry: ModelRegistry,
    downloader: DownloadManager,
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
            hardware,
            runtime: LlamaCppRuntime::new(llama_config),
        })
    }

    pub fn with_download_options(
        vault_path: impl AsRef<Path>,
        download_options: DownloadOptions,
    ) -> ModelResult<Self> {
        let mut mgr = Self::new(vault_path)?;
        mgr.downloader = DownloadManager::new(download_options);
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

    /// Curated catalog for browse UI.
    pub fn browse_catalog(&self) -> Vec<ModelCatalogEntry> {
        curated_catalog()
    }

    /// Import an existing local GGUF file into the vault registry.
    pub fn import_local(&mut self, name: impl Into<String>, path: impl AsRef<Path>) -> ModelResult<ModelEntry> {
        let path = path.as_ref();
        let entry = self.registry.register_local(name, path)?;
        self.persist()?;
        info!(id = %entry.id, path = %path.display(), "imported local model");
        Ok(entry)
    }

    /// Install from catalog entry id (HuggingFace download or Ollama registration).
    #[instrument(skip(self))]
    pub async fn install_catalog(
        &mut self,
        catalog_id: &str,
        ollama_base_url: Option<String>,
    ) -> ModelResult<ModelEntry> {
        let catalog = find_catalog_entry(catalog_id)
            .ok_or_else(|| ModelError::not_found(format!("catalog entry: {catalog_id}")))?;

        match catalog.provider {
            ModelProvider::Ollama => {
                let tag = catalog
                    .ollama_tag
                    .ok_or_else(|| ModelError::invalid("catalog entry missing ollama tag"))?;
                let base_url = ollama_base_url.unwrap_or_else(|| "http://127.0.0.1:11434".into());
                let runtime = OllamaRuntime::new(OllamaConfig {
                    base_url: base_url.clone(),
                    model: tag.clone(),
                });
                runtime.pull_model().await?;
                let entry = self.registry.register_ollama(
                    &self.vault_path,
                    catalog.name,
                    tag,
                    base_url,
                )?;
                self.persist()?;
                Ok(entry)
            }
            ModelProvider::HuggingFace => {
                let repo = catalog
                    .repo
                    .ok_or_else(|| ModelError::invalid("catalog entry missing repo"))?;
                let filename = catalog
                    .filename
                    .ok_or_else(|| ModelError::invalid("catalog entry missing filename"))?;
                self.download_huggingface(HuggingFaceDownloadRequest {
                    name: catalog.name,
                    repo,
                    filename,
                    revision: Some(catalog.version.clone()),
                    expected_sha256: None,
                    expected_size_bytes: catalog.size_bytes,
                })
                .await
            }
            ModelProvider::Gguf => Err(ModelError::invalid(
                "catalog GGUF entries must be installed via HuggingFace or import",
            )),
        }
    }

    /// Download a GGUF model from HuggingFace, verify, and register.
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

    pub fn get_model(&self, model_id: &str) -> Option<&ModelEntry> {
        self.registry.get(model_id)
    }
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
    fn browse_returns_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = LocalModelManager::new(dir.path().join("vault")).unwrap();
        assert!(!mgr.browse_catalog().is_empty());
    }
}
