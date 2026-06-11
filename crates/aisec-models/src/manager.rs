use std::path::{Path, PathBuf};

use time::OffsetDateTime;
use tracing::{info, instrument};
use uuid::Uuid;

use crate::download::{DownloadManager, DownloadOptions};
use crate::error::{ModelError, ModelResult};
use crate::hardware::detect_hardware;
use crate::registry::ModelRegistry;
use crate::runtime::{InferenceRuntime, LlamaCppConfig, LlamaCppRuntime};
use crate::types::{
    DownloadStatus, HardwareProfile, HuggingFaceDownloadRequest, InferenceRequest,
    InferenceResponse, ModelEntry, ModelFormat, ModelSource, VerificationResult,
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

        Ok(Self {
            vault_path,
            registry: ModelRegistry::new(),
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

    /// Import an existing local GGUF file into the vault registry.
    pub fn import_local(&mut self, name: impl Into<String>, path: impl AsRef<Path>) -> ModelResult<ModelEntry> {
        let path = path.as_ref();
        let entry = self.registry.register_local(name, path)?;
        info!(id = %entry.id, path = %path.display(), "imported local model");
        Ok(entry)
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
        let url = crate::download::huggingface_url(
            &request.repo,
            &request.filename,
            request.revision.as_deref(),
        );

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

        let now = OffsetDateTime::now_utc();
        let entry = ModelEntry {
            id: model_id.clone(),
            name: request.name,
            format: ModelFormat::Gguf,
            source: ModelSource::HuggingFace {
                repo: request.repo,
                filename: request.filename.clone(),
                revision: request.revision,
            },
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
        info!(id = %entry.id, "model registered after download");
        Ok(entry)
    }

    /// Verify a registered model's SHA256 checksum.
    pub async fn verify_model(&mut self, model_id: &str) -> ModelResult<VerificationResult> {
        let entry = self
            .registry
            .get(model_id)
            .ok_or_else(|| ModelError::not_found(model_id))?
            .clone();

        let expected = entry.checksum_sha256.as_deref();
        let result = self.verify_file(&entry.file_path, expected).await?;

        if result.valid {
            self.registry.update_verification(
                model_id,
                result.actual_sha256.clone(),
                true,
            )?;
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
    pub async fn complete(&self, request: InferenceRequest) -> ModelResult<InferenceResponse> {
        self.runtime.complete(request).await
    }

    pub fn list_models(&self) -> Vec<&ModelEntry> {
        self.registry.list()
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
}
