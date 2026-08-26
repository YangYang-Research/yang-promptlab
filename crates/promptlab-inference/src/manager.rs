use std::path::Path;
use std::sync::Arc;

use promptlab_models::{ModelEntry, ModelProvider, ModelSource};
use promptlab_runtime::{RuntimeManager, SharedModelProvider};

use crate::capabilities::ModelCapabilities;
use crate::config::{AiRuntimeConfiguration, InferenceMode, InferenceProvider, load_config, save_config};
use crate::error::{InferenceError, InferenceResult};
use crate::provider::{ProviderAdapter, RemoteAdapterSettings, RemoteProviderAdapter};
use crate::types::HealthStatus;

/// Central AI runtime orchestrator — config, lifecycle coordination, provider selection.
pub struct InferenceRuntimeManager {
    data_dir: std::path::PathBuf,
    config: AiRuntimeConfiguration,
}

impl InferenceRuntimeManager {
    pub fn new(data_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            config: AiRuntimeConfiguration::default(),
        }
    }

    pub fn config(&self) -> &AiRuntimeConfiguration {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut AiRuntimeConfiguration {
        &mut self.config
    }

    pub async fn load(&mut self) -> InferenceResult<()> {
        self.config = load_config(&self.data_dir).await?;
        // Migrate legacy local/embedded/ollama runtime labels to remote cloud route.
        if matches!(
            self.config.runtime.as_str(),
            "local" | "embedded" | "ollama"
        ) {
            self.config.provider = InferenceProvider::OpenAi;
            self.config.runtime = "cloud".into();
            self.config.status = "not_configured".into();
            self.config.initialized = false;
            self.config.model.clear();
            self.config.selected_model_id = None;
            let _ = self.save().await;
        }
        Ok(())
    }

    pub async fn save(&self) -> InferenceResult<()> {
        save_config(&self.data_dir, &self.config).await
    }

    pub fn is_ready(&self) -> bool {
        self.config.initialized
            && self.config.mode != InferenceMode::Deterministic
            && (!self.config.model.is_empty() || self.config.selected_model_id.is_some())
    }

    pub async fn update_from_model(
        &mut self,
        entry: &ModelEntry,
        remote: Option<RemoteAdapterSettings>,
    ) -> InferenceResult<()> {
        self.config.selected_model_id = Some(entry.id.clone());
        self.config.model = entry.display_model_name();
        match entry.provider {
            ModelProvider::Remote => {
                self.config.mode = InferenceMode::ThirdParty;
                let remote = remote.ok_or_else(|| {
                    InferenceError::Config("third-party model requires credentials".into())
                })?;
                self.config.provider = remote.provider;
                self.config.runtime = "cloud".into();
            }
        }
        self.config.initialized = true;
        self.config.status = "configured".into();
        self.save().await
    }

    pub async fn build_provider_adapter(
        &self,
        _entry: &ModelEntry,
        remote: Option<RemoteAdapterSettings>,
        _model_provider: SharedModelProvider,
        _runtime_manager: &mut RuntimeManager,
    ) -> InferenceResult<Arc<dyn ProviderAdapter>> {
        match self.config.mode {
            InferenceMode::Deterministic => Err(InferenceError::NotReady(
                "deterministic mode has no LLM provider".into(),
            )),
            InferenceMode::ThirdParty => {
                let settings = remote.ok_or_else(|| {
                    InferenceError::Config("missing remote credentials".into())
                })?;
                Ok(Arc::new(RemoteProviderAdapter::new(settings)))
            }
        }
    }

    pub fn capabilities_for(&self, adapter: &dyn ProviderAdapter) -> ModelCapabilities {
        if self.config.mode == InferenceMode::Deterministic {
            return ModelCapabilities::deterministic();
        }
        adapter.capabilities()
    }

    pub async fn health_check(
        &self,
        adapter: &dyn ProviderAdapter,
    ) -> InferenceResult<HealthStatus> {
        let started = std::time::Instant::now();
        let ok = adapter.health().await.unwrap_or(false);
        Ok(HealthStatus {
            ok,
            provider: adapter.provider_id().into(),
            model: adapter.model_id().into(),
            message: if ok {
                "healthy".into()
            } else {
                "unhealthy".into()
            },
            latency_ms: started.elapsed().as_millis() as u64,
        })
    }

    pub fn remote_settings_from_entry(
        entry: &ModelEntry,
        api_key: String,
        aws_secret: Option<String>,
        aws_session: Option<String>,
    ) -> InferenceResult<RemoteAdapterSettings> {
        match &entry.source {
            ModelSource::Remote {
                provider,
                model,
                base_url,
                region,
            } => Ok(RemoteAdapterSettings {
                provider: InferenceProvider::parse(provider),
                model: model.clone(),
                base_url: base_url.clone(),
                api_key,
                aws_secret_access_key: aws_secret,
                aws_region: region.clone(),
                aws_session_token: aws_session,
            }),
        }
    }
}

pub fn config_path(data_dir: &Path) -> std::path::PathBuf {
    crate::config::config_path(data_dir)
}
