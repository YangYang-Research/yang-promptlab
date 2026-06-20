use std::path::{Path, PathBuf};

use time::OffsetDateTime;
use tracing::{info, instrument, warn};
use uuid::Uuid;

use crate::builtin_catalog::BuiltinCatalog;
use crate::catalog::find_catalog_entry;
use crate::download::{DownloadCoordinator, DownloadManager, DownloadOptions, PipelinePhase, ResumeState};
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

pub struct FinalizePlan {
    pub catalog_id: String,
    pub destination: PathBuf,
    pub catalog: ModelCatalogEntry,
    pub progress: DownloadProgress,
}

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

        if let Some((destination, state)) =
            Self::find_pipeline_for_catalog(&self.vault_path, catalog_id, &self.catalog)
        {
            match state.phase {
                PipelinePhase::Downloaded
                | PipelinePhase::VerifyFailed
                | PipelinePhase::Verifying => {
                    return self
                        .resume_catalog_verify(catalog_id, destination, state, true)
                        .await;
                }
                PipelinePhase::Downloading => {
                    if let Some(parent) = destination.parent() {
                        tokio::fs::create_dir_all(parent).await.map_err(ModelError::Io)?;
                    }
                    return self
                        .download_coordinator
                        .start_url_download(
                            catalog_id,
                            url,
                            destination,
                            catalog.sha256.clone(),
                            catalog.size_bytes,
                        )
                        .await;
                }
            }
        }

        let destination = Self::find_resumable_destination(&self.vault_path, url, &filename)
            .unwrap_or_else(|| {
                let model_id = Uuid::new_v4().to_string();
                ModelRegistry::model_dir(&self.vault_path, &model_id).join(&filename)
            });

        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(ModelError::Io)?;
        }

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

    /// Progress snapshot from on-disk pipeline when no in-memory download slot exists.
    pub async fn persisted_pipeline_progress(&self) -> Option<DownloadProgress> {
        let (catalog_id, destination, state) =
            Self::scan_first_pipeline(&self.vault_path, &self.catalog)?;
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
            Self::find_pipeline_for_catalog(&self.vault_path, catalog_id, &self.catalog)
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
            Self::scan_first_pipeline(&self.vault_path, &self.catalog)
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
        if let Some((catalog_id, destination, progress)) =
            self.download_coordinator.snapshot_if_verifying().await
        {
            if let Ok(Some(state)) = DownloadManager::load_pipeline_state(&destination).await {
                if state.phase != PipelinePhase::Verifying || state.verify_manual {
                    self.download_coordinator.mark_awaiting_verify().await;
                    return Ok(None);
                }
            }
            let on_disk = tokio::fs::metadata(&destination)
                .await
                .map(|meta| meta.len())
                .unwrap_or(progress.downloaded_bytes);
            if !pipeline_download_complete(on_disk, progress.total_bytes) {
                warn!(
                    catalog_id = %catalog_id,
                    on_disk,
                    total = ?progress.total_bytes,
                    "stuck verifying with incomplete file; resuming download"
                );
                self.resume_incomplete_catalog_download(&catalog_id, destination)
                    .await?;
                return Ok(None);
            }
            let catalog = self
                .find_catalog_entry(&catalog_id)
                .ok_or_else(|| ModelError::not_found(format!("catalog entry: {catalog_id}")))?
                .clone();
            return Ok(Some(FinalizePlan {
                catalog_id,
                destination,
                catalog,
                progress,
            }));
        }

        let Some((catalog_id, destination, progress)) =
            self.download_coordinator.snapshot_if_completed().await
        else {
            return Ok(None);
        };

        let on_disk = tokio::fs::metadata(&destination)
            .await
            .map(|meta| meta.len())
            .unwrap_or(progress.downloaded_bytes);
        if !pipeline_download_complete(on_disk, progress.total_bytes) {
            warn!(
                catalog_id = %catalog_id,
                on_disk,
                total = ?progress.total_bytes,
                "download incomplete; resuming HTTP before verify"
            );
            self.resume_incomplete_catalog_download(&catalog_id, destination)
                .await?;
            return Ok(None);
        }

        if let Ok(Some(state)) = DownloadManager::load_pipeline_state(&destination).await {
            if state.verify_manual {
                self.download_coordinator.mark_awaiting_verify().await;
                return Ok(None);
            }
        }

        self.download_coordinator.mark_verifying().await;
        DownloadManager::update_pipeline_phase(&destination, PipelinePhase::Verifying, None)
            .await?;

        let catalog = self
            .find_catalog_entry(&catalog_id)
            .ok_or_else(|| ModelError::not_found(format!("catalog entry: {catalog_id}")))?
            .clone();

        Ok(Some(FinalizePlan {
            catalog_id,
            destination,
            catalog,
            progress,
        }))
    }

    async fn resume_incomplete_catalog_download(
        &mut self,
        catalog_id: &str,
        destination: PathBuf,
    ) -> ModelResult<DownloadProgress> {
        let catalog = self
            .find_catalog_entry(catalog_id)
            .ok_or_else(|| ModelError::not_found(format!("catalog entry: {catalog_id}")))?
            .clone();
        let url = catalog
            .download_url
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ModelError::invalid("catalog entry missing download_url"))?;
        DownloadManager::update_pipeline_phase(&destination, PipelinePhase::Downloading, None)
            .await?;
        self.download_coordinator
            .restart_url_download(
                catalog_id,
                url,
                destination,
                catalog.sha256.clone(),
                catalog.size_bytes,
            )
            .await
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
            metadata: serde_json::json!({ "download": progress }),
        };

        self.registry.register_entry(entry.clone())?;
        self.persist()?;
        Ok(Some(entry))
    }

    /// Register completed GGUF files in the vault that match the built-in catalog but are missing from registry.json.
    pub async fn recover_orphan_downloads(&mut self) -> ModelResult<usize> {
        use std::collections::HashSet;

        let registered_paths: HashSet<PathBuf> = self
            .registry
            .list()
            .iter()
            .map(|entry| entry.file_path.clone())
            .collect();
        let registered_names: HashSet<String> = self
            .registry
            .list()
            .iter()
            .map(|entry| entry.name.clone())
            .collect();

        let mut recovered = 0usize;
        let vault = self.vault_path.clone();

        for gguf_path in walk_vault_files(&vault, |path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("gguf"))
        }) {
            if registered_paths.contains(&gguf_path) {
                continue;
            }

            let filename = gguf_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string();
            let Some(catalog) = self
                .catalog
                .iter()
                .find(|entry| {
                    entry.filename.as_deref() == Some(filename.as_str())
                        || entry
                            .download_url
                            .as_ref()
                            .is_some_and(|url| url.ends_with(&filename))
                })
                .cloned()
            else {
                continue;
            };

            if registered_names.contains(&catalog.name) {
                continue;
            }

            let pipeline_path = gguf_path.with_extension("download.json");
            if pipeline_path.is_file() {
                continue;
            }

            let expected_sha256 = catalog.sha256.as_deref().filter(|s| !s.is_empty());
            let verification = self.verify_file(&gguf_path, expected_sha256).await?;
            if !verification.valid {
                continue;
            }

            let source = ModelSource::Local {
                path: gguf_path.clone(),
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
                file_path: gguf_path,
                size_bytes: Some(verification.size_bytes),
                checksum_sha256: Some(verification.actual_sha256),
                verified: true,
                created_at: now,
                updated_at: now,
                metadata: serde_json::json!({ "recovered": true }),
            };

            self.registry.register_entry(entry)?;
            recovered += 1;
        }

        if recovered > 0 {
            self.persist()?;
            info!(recovered, "recovered orphan catalog downloads");
        }

        Ok(recovered)
    }

    /// Restore persisted download / verify pipeline after restart.
    pub async fn restore_persisted_pipelines(&mut self) -> ModelResult<()> {
        if self.download_coordinator.status().await.is_some() {
            return Ok(());
        }

        let Some((catalog_id, destination, state)) =
            Self::scan_first_pipeline(&self.vault_path, &self.catalog)
        else {
            return Ok(());
        };

        match state.phase {
            PipelinePhase::Downloading => {
                let on_disk = tokio::fs::metadata(&destination)
                    .await
                    .map(|meta| meta.len())
                    .unwrap_or(state.downloaded_bytes);
                if pipeline_download_complete(on_disk, state.total_bytes) {
                    info!(
                        catalog_id = %catalog_id,
                        on_disk,
                        "download complete on disk; verifying without re-download"
                    );
                    DownloadManager::update_pipeline_phase(
                        &destination,
                        PipelinePhase::Downloaded,
                        None,
                    )
                    .await?;
                    let mut state = state;
                    state.downloaded_bytes = on_disk;
                    state.phase = PipelinePhase::Downloaded;
                    self.resume_catalog_verify(&catalog_id, destination, state, true)
                        .await?;
                } else {
                    info!(catalog_id = %catalog_id, path = %destination.display(), "resuming interrupted download");
                    let catalog = self
                        .find_catalog_entry(&catalog_id)
                        .ok_or_else(|| {
                            ModelError::not_found(format!("catalog entry: {catalog_id}"))
                        })?
                        .clone();
                    let url = catalog.download_url.as_deref().unwrap_or(&state.url);
                    self.download_coordinator
                        .start_url_download(
                            catalog_id,
                            url,
                            destination,
                            catalog.sha256.clone(),
                            catalog.size_bytes,
                        )
                        .await?;
                }
            }
            PipelinePhase::Downloaded | PipelinePhase::VerifyFailed | PipelinePhase::Verifying => {
                info!(catalog_id = %catalog_id, phase = ?state.phase, "restored download awaiting verify");
                let on_disk = tokio::fs::metadata(&destination)
                    .await
                    .map(|meta| meta.len())
                    .unwrap_or(state.downloaded_bytes);
                if !pipeline_download_complete(on_disk, state.total_bytes) {
                    warn!(
                        catalog_id = %catalog_id,
                        on_disk,
                        "pipeline marked post-download but file incomplete; resuming HTTP"
                    );
                    DownloadManager::update_pipeline_phase(
                        &destination,
                        PipelinePhase::Downloading,
                        None,
                    )
                    .await?;
                    let catalog = self
                        .find_catalog_entry(&catalog_id)
                        .ok_or_else(|| {
                            ModelError::not_found(format!("catalog entry: {catalog_id}"))
                        })?
                        .clone();
                    let url = catalog.download_url.as_deref().unwrap_or(&state.url);
                    self.download_coordinator
                        .start_url_download(
                            catalog_id,
                            url,
                            destination,
                            catalog.sha256.clone(),
                            catalog.size_bytes,
                        )
                        .await?;
                    return Ok(());
                }
                if state.phase == PipelinePhase::Verifying {
                    DownloadManager::update_pipeline_phase(
                        &destination,
                        PipelinePhase::Downloaded,
                        None,
                    )
                    .await?;
                }
                self.resume_catalog_verify(&catalog_id, destination, state, true)
                    .await?;
            }
        }
        Ok(())
    }

    fn find_pipeline_for_catalog(
        vault: &Path,
        catalog_id: &str,
        catalog: &[ModelCatalogEntry],
    ) -> Option<(PathBuf, ResumeState)> {
        Self::scan_all_pipelines(vault, catalog)
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
        catalog: &[ModelCatalogEntry],
    ) -> Option<(String, PathBuf, ResumeState)> {
        Self::scan_all_pipelines(vault, catalog).into_iter().next()
    }

    fn scan_all_pipelines(
        vault: &Path,
        catalog: &[ModelCatalogEntry],
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
            let catalog_id = state.catalog_id.clone().filter(|id| !id.is_empty()).or_else(|| {
                catalog
                    .iter()
                    .find(|entry| entry.download_url.as_deref() == Some(state.url.as_str()))
                    .map(|entry| entry.id.clone())
            });
            let Some(catalog_id) = catalog_id else {
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
