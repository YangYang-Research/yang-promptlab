use std::path::{Path, PathBuf};

use time::OffsetDateTime;
use tracing::{info, instrument, warn};
use uuid::Uuid;

use crate::download::{DownloadCoordinator, DownloadManager, DownloadOptions, PipelinePhase, ResumeState};
use crate::import_pack::{extract_gguf_from_zip, validate_gguf_path};
use crate::error::{ModelError, ModelResult};
use crate::hardware::detect_hardware;
use crate::registry::ModelRegistry;
use crate::runtime::{
    infer_capabilities, infer_provider, infer_version, LocalInferenceEngine,
};
use crate::types::{
    ChatMessage, ChatRequest, DownloadProgress, DownloadStatus, HardwareProfile, HuggingFaceDownloadRequest,
    InferenceRequest, ModelCatalogEntry, ModelEntry, ModelFormat, ModelProvider, ModelSource,
    VerificationResult,
};
use crate::verify::VerificationEngine;

pub struct FinalizePlan {
    pub catalog_id: String,
    pub destination: PathBuf,
    pub catalog: ModelCatalogEntry,
    pub progress: DownloadProgress,
}

/// Top-level model manager orchestrating registry, downloads, verification, and remote providers.
pub struct LocalModelManager {
    vault_path: PathBuf,
    registry: ModelRegistry,
    downloader: DownloadManager,
    download_coordinator: DownloadCoordinator,
    hardware: HardwareProfile,
}

impl LocalModelManager {
    pub fn new(vault_path: impl AsRef<Path>) -> ModelResult<Self> {
        let vault_path = vault_path.as_ref().to_path_buf();
        std::fs::create_dir_all(&vault_path).map_err(ModelError::Io)?;

        let hardware = detect_hardware()?;
        let mut registry = ModelRegistry::load_from_vault(&vault_path)?;
        if ModelRegistry::migrate_storage_layout(&vault_path, &mut registry)? {
            registry.save_to_vault(&vault_path)?;
        }

        Ok(Self {
            vault_path,
            registry,
            downloader: DownloadManager::with_defaults(),
            download_coordinator: DownloadCoordinator::new(DownloadManager::with_defaults()),
            hardware,
        })
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

    fn persist(&self) -> ModelResult<()> {
        self.registry.save_to_vault(&self.vault_path)
    }

    pub fn download_coordinator(&self) -> &DownloadCoordinator {
        &self.download_coordinator
    }

    /// Built-in GGUF catalog removed — always empty (Remote-only).
    pub fn browse_catalog(&self) -> &[ModelCatalogEntry] {
        &[]
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

    /// Built-in GGUF catalog removed.
    pub async fn install_catalog(
        &mut self,
        _catalog_id: &str,
        _ollama_base_url: Option<String>,
    ) -> ModelResult<ModelEntry> {
        Err(ModelError::invalid(
            "builtin GGUF catalog has been removed — add a remote third-party provider instead",
        ))
    }

    /// Built-in GGUF catalog removed.
    pub async fn start_catalog_download(
        &mut self,
        _catalog_id: &str,
    ) -> ModelResult<DownloadProgress> {
        Err(ModelError::invalid(
            "builtin GGUF catalog has been removed — add a remote third-party provider instead",
        ))
    }

    pub async fn download_status(&self) -> Option<DownloadProgress> {
        self.download_coordinator.status().await
    }

    /// Progress snapshot from on-disk pipeline when no in-memory download slot exists.
    pub async fn persisted_pipeline_progress(&self) -> Option<DownloadProgress> {
        let (catalog_id, destination, state) =
            Self::scan_first_pipeline(&self.vault_path)?;
        let on_disk = tokio::fs::metadata(&destination)
            .await
            .map(|meta| meta.len())
            .unwrap_or(state.downloaded_bytes);
        let status = match state.phase {
            PipelinePhase::Downloading => DownloadStatus::Downloading,
            PipelinePhase::Downloaded => {
                if state.verify_manual {
                    DownloadStatus::AwaitingVerify
                } else {
                    DownloadStatus::Completed
                }
            }
            PipelinePhase::Verifying => DownloadStatus::Verifying,
            PipelinePhase::VerifyFailed => DownloadStatus::VerifyFailed,
        };
        Some(DownloadProgress {
            model_id: catalog_id,
            status,
            url: state.url,
            destination,
            downloaded_bytes: on_disk,
            total_bytes: state.total_bytes,
            speed_bytes_per_sec: None,
            eta_seconds: None,
            resumed: true,
            updated_at: state.updated_at,
            error: state.error,
        })
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
        let Some(plan) = self.prepare_finalize().await? else {
            return Ok(None);
        };
        let expected_sha256 = plan.catalog.sha256.as_deref().filter(|s| !s.is_empty());
        let verification =
            VerificationEngine::verify_file(&plan.destination, expected_sha256).await?;
        self.complete_finalize(plan, verification).await
    }

    pub async fn retry_catalog_verify(&mut self, catalog_id: &str) -> ModelResult<DownloadProgress> {
        self.begin_catalog_verify(catalog_id).await
    }

    /// Mark verify in-flight immediately; caller should run finalize in the background.
    pub async fn begin_catalog_verify(&mut self, catalog_id: &str) -> ModelResult<DownloadProgress> {
        let Some((destination, state)) =
            Self::find_pipeline_for_catalog(&self.vault_path, catalog_id)
        else {
            return Err(ModelError::not_found(format!(
                "no downloaded model awaiting verify: {catalog_id}"
            )));
        };
        let on_disk = tokio::fs::metadata(&destination)
            .await
            .map(|meta| meta.len())
            .unwrap_or(state.downloaded_bytes);
        if !pipeline_download_complete(on_disk, state.total_bytes) {
            return Err(ModelError::invalid(
                "download incomplete; finish downloading before verify",
            ));
        }
        DownloadManager::set_verify_manual(&destination, false).await?;
        DownloadManager::update_pipeline_phase(&destination, PipelinePhase::Downloaded, None).await?;
        self.resume_catalog_verify(catalog_id, destination.clone(), state, true)
            .await?;
        self.download_coordinator.mark_verifying().await;
        DownloadManager::update_pipeline_phase(&destination, PipelinePhase::Verifying, None).await?;
        self.download_coordinator.status().await.ok_or_else(|| {
            ModelError::invalid("download slot missing after starting verify")
        })
    }

    pub async fn cancel_catalog_verify(&mut self) -> ModelResult<DownloadProgress> {
        if let Some(progress) = self.download_coordinator.status().await {
            if progress.status == DownloadStatus::Verifying {
                DownloadManager::set_verify_manual(&progress.destination, true).await?;
                DownloadManager::update_pipeline_phase(
                    &progress.destination,
                    PipelinePhase::Downloaded,
                    None,
                )
                .await?;
                self.download_coordinator.mark_awaiting_verify().await;
                return self.download_coordinator.status().await.ok_or_else(|| {
                    ModelError::invalid("download slot missing after cancelling verify")
                });
            }
        }

        let Some((catalog_id, destination, state)) =
            Self::scan_first_pipeline(&self.vault_path)
        else {
            return Err(ModelError::invalid("no active download to cancel"));
        };
        if state.phase != PipelinePhase::Verifying {
            return Err(ModelError::invalid("download is not verifying"));
        }
        let on_disk = tokio::fs::metadata(&destination)
            .await
            .map(|meta| meta.len())
            .unwrap_or(state.downloaded_bytes);
        DownloadManager::set_verify_manual(&destination, true).await?;
        DownloadManager::update_pipeline_phase(&destination, PipelinePhase::Downloaded, None).await?;
        let progress = DownloadProgress {
            model_id: catalog_id.clone(),
            status: DownloadStatus::AwaitingVerify,
            url: state.url,
            destination: destination.clone(),
            downloaded_bytes: on_disk,
            total_bytes: state.total_bytes,
            speed_bytes_per_sec: None,
            eta_seconds: Some(0),
            resumed: true,
            updated_at: OffsetDateTime::now_utc(),
            error: None,
        };
        if self.download_coordinator.status().await.is_some() {
            self.download_coordinator.mark_awaiting_verify().await;
            self.download_coordinator.status().await.ok_or_else(|| {
                ModelError::invalid("download slot missing after cancelling verify")
            })
        } else {
            self.download_coordinator
                .restore_post_download(catalog_id, destination, progress)
                .await
        }
    }

    async fn resume_catalog_verify(
        &mut self,
        catalog_id: &str,
        destination: PathBuf,
        state: ResumeState,
        retry_verify: bool,
    ) -> ModelResult<DownloadProgress> {
        let on_disk = tokio::fs::metadata(&destination)
            .await
            .map(|meta| meta.len())
            .unwrap_or(state.downloaded_bytes);
        let status = if retry_verify {
            DownloadStatus::Completed
        } else if state.phase == PipelinePhase::VerifyFailed {
            DownloadStatus::VerifyFailed
        } else {
            DownloadStatus::Completed
        };
        let progress = DownloadProgress {
            model_id: catalog_id.to_string(),
            status,
            url: state.url,
            destination: destination.clone(),
            downloaded_bytes: on_disk,
            total_bytes: state.total_bytes,
            speed_bytes_per_sec: None,
            eta_seconds: Some(0),
            resumed: true,
            updated_at: OffsetDateTime::now_utc(),
            error: if status == DownloadStatus::VerifyFailed {
                state.error.clone()
            } else {
                None
            },
        };
        self.download_coordinator
            .restore_post_download(catalog_id, destination, progress)
            .await
    }

    pub async fn prepare_finalize(&mut self) -> ModelResult<Option<FinalizePlan>> {
        // Built-in catalog removed — no GGUF finalize path.
        let _ = self;
        Ok(None)
    }

    pub async fn record_verify_error(
        &mut self,
        destination: &Path,
        message: impl Into<String>,
    ) -> ModelResult<()> {
        let message = message.into();
        DownloadManager::update_pipeline_phase(
            destination,
            PipelinePhase::VerifyFailed,
            Some(message.clone()),
        )
        .await?;
        self.download_coordinator.mark_verify_failed(message).await;
        Ok(())
    }

    pub async fn complete_finalize(
        &mut self,
        plan: FinalizePlan,
        verification: VerificationResult,
    ) -> ModelResult<Option<ModelEntry>> {
        if let Ok(Some(state)) = DownloadManager::load_pipeline_state(&plan.destination).await {
            if state.phase != PipelinePhase::Verifying {
                self.download_coordinator.mark_awaiting_verify().await;
                return Ok(None);
            }
        }

        if !verification.valid {
            let message = "post-download checksum mismatch".to_string();
            DownloadManager::update_pipeline_phase(
                &plan.destination,
                PipelinePhase::VerifyFailed,
                Some(message.clone()),
            )
            .await?;
            self.download_coordinator.mark_verify_failed(message).await;
            return Ok(None);
        }

        DownloadManager::clear_pipeline_state(&plan.destination).await?;
        self.download_coordinator.consume_completed().await;

        let destination = plan.destination;
        let catalog = plan.catalog;
        let progress = plan.progress;

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
            metadata: serde_json::json!({
                "download": progress,
                "registry_id": catalog.id,
                "providerLabel": catalog.provider_label,
            }),
        };

        self.registry.register_entry(entry.clone())?;
        self.persist()?;
        Ok(Some(entry))
    }

    /// Built-in catalog removed — no orphan GGUF recovery.
    pub async fn recover_orphan_downloads(&mut self) -> ModelResult<usize> {
        let _ = self;
        Ok(0)
    }

    /// Built-in catalog removed — skip GGUF pipeline restore.
    pub async fn restore_persisted_pipelines(&mut self) -> ModelResult<()> {
        let _ = self;
        Ok(())
    }

    fn find_pipeline_for_catalog(
        vault: &Path,
        catalog_id: &str,
    ) -> Option<(PathBuf, ResumeState)> {
        Self::scan_all_pipelines(vault)
            .into_iter()
            .find_map(|(id, dest, state)| {
                if id == catalog_id {
                    Some((dest, state))
                } else {
                    None
                }
            })
    }

    fn scan_first_pipeline(
        vault: &Path,
    ) -> Option<(String, PathBuf, ResumeState)> {
        Self::scan_all_pipelines(vault).into_iter().next()
    }

    fn scan_all_pipelines(
        vault: &Path,
    ) -> Vec<(String, PathBuf, ResumeState)> {
        let mut items = Vec::new();
        for path in walk_vault_files(vault, |path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".download.json"))
        }) {
            let Ok(raw) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(mut state) = serde_json::from_str::<ResumeState>(&raw) else {
                continue;
            };
            let Some(catalog_id) = state.catalog_id.clone().filter(|id| !id.is_empty()) else {
                continue;
            };
            state.catalog_id = Some(catalog_id.clone());
            items.push((catalog_id, state.destination.clone(), state));
        }
        items
    }

    fn find_resumable_destination(vault: &Path, url: &str, _filename: &str) -> Option<PathBuf> {
        for path in walk_vault_files(vault, |path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".download.json"))
        }) {
            let Ok(raw) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(state) = serde_json::from_str::<ResumeState>(&raw) else {
                continue;
            };
            if state.url == url {
                return Some(state.destination);
            }
        }
        None
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
            let engine = LocalInferenceEngine::from_entry(entry.clone()).await?;
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

    /// Build a unified inference engine for a registered model (Ollama HTTP only).
    pub async fn inference_engine(&self, model_id: &str) -> ModelResult<LocalInferenceEngine> {
        let entry = self
            .registry
            .get(model_id)
            .ok_or_else(|| ModelError::not_found(model_id))?
            .clone();
        LocalInferenceEngine::from_entry(entry).await
    }

    /// Run a completion smoke test on a registered model.
    pub async fn test_inference(&self, model_id: &str) -> ModelResult<String> {
        let engine = self.inference_engine(model_id).await?;
        let response = engine
            .complete(InferenceRequest {
                system: None,
                prompt: "Reply with exactly: PromptLab OK".into(),
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
                    content: "Reply with exactly: PromptLab OK".into(),
                }],
                max_tokens: 16,
                temperature: 0.0,
            })
            .await?;
        Ok(response.message.content)
    }

    /// Embedded GGUF load is no longer supported.
    pub async fn load(&mut self, _model_id: &str) -> ModelResult<()> {
        Err(ModelError::invalid(
            "embedded GGUF / llama.cpp runtime has been removed — use a remote provider or Ollama over HTTP",
        ))
    }

    /// No-op: there is no in-process model to unload.
    pub async fn unload(&mut self) -> ModelResult<()> {
        Ok(())
    }

    /// In-process completion is no longer supported.
    pub async fn complete(
        &self,
        _request: InferenceRequest,
    ) -> ModelResult<crate::types::InferenceResponse> {
        Err(ModelError::invalid(
            "embedded GGUF / llama.cpp runtime has been removed — use a remote provider or Ollama over HTTP",
        ))
    }

    pub fn list_models(&self) -> Vec<&ModelEntry> {
        self.registry.list()
    }

    /// Aggregate installed model sizes for desktop UI cards.
    pub fn vault_stats(&self) -> ModelResult<crate::types::VaultStats> {
        let models = self.list_models();
        let local_models: Vec<_> = models
            .iter()
            .filter(|entry| entry.provider != ModelProvider::Remote)
            .collect();
        let installed_bytes = local_models
            .iter()
            .filter_map(|entry| entry.size_bytes)
            .sum();
        Ok(crate::types::VaultStats {
            registered_count: models.len(),
            installed_local_count: local_models.len(),
            installed_bytes,
            vault_path: self.vault_path.clone(),
        })
    }

    pub fn update_model_metadata(
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
        self.persist()?;
        Ok(self.registry.get(model_id).expect("entry exists"))
    }

    pub fn set_model_verified(&mut self, model_id: &str, verified: bool) -> ModelResult<&ModelEntry> {
        let entry = self
            .registry
            .get_mut(model_id)
            .ok_or_else(|| ModelError::not_found(model_id))?;
        entry.verified = verified;
        entry.updated_at = OffsetDateTime::now_utc();
        self.persist()?;
        Ok(self.registry.get(model_id).expect("entry exists"))
    }

    pub fn get_model(&self, model_id: &str) -> Option<&ModelEntry> {
        self.registry.get(model_id)
    }
}

fn walk_vault_files(vault: &Path, mut matches: impl FnMut(&Path) -> bool) -> Vec<PathBuf> {
    fn walk(
        dir: &Path,
        matches: &mut dyn FnMut(&Path) -> bool,
        out: &mut Vec<PathBuf>,
    ) {
        let Ok(read_dir) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, matches, out);
            } else if matches(&path) {
                out.push(path);
            }
        }
    }

    let mut out = Vec::new();
    walk(vault, &mut matches, &mut out);
    out
}

fn vault_near_complete(on_disk: u64, total: Option<u64>) -> bool {
    let Some(total) = total.filter(|value| *value > 0) else {
        return false;
    };
    let remaining = total.saturating_sub(on_disk);
    remaining <= 1024 || remaining * 100 <= total
}

fn pipeline_download_complete(on_disk: u64, total: Option<u64>) -> bool {
    total.is_some_and(|total| total > 0 && on_disk + 1024 >= total)
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

}
