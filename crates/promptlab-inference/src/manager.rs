use std::path::Path;
use std::sync::Arc;

use aisec_models::{ModelEntry, ModelProvider, ModelSource};
use aisec_runtime::{ModelProviderRuntime, RuntimeManager, SharedModelProvider};
use tokio::sync::Mutex;

use crate::capabilities::ModelCapabilities;
use crate::config::{AiRuntimeConfiguration, InferenceMode, InferenceProvider, load_config, save_config};
use crate::error::{InferenceError, InferenceResult};
use crate::provider::{
    LlamaCppAdapter, ProviderAdapter, RemoteAdapterSettings, RemoteProviderAdapter,
};
use crate::runtime::{LocalRuntimeAdapterBridge, RuntimeAdapter};
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
            ModelProvider::Ollama => {
                self.config.mode = InferenceMode::Local;
                self.config.provider = InferenceProvider::Ollama;
                self.config.runtime = "ollama".into();
            }
            _ => {
                self.config.mode = InferenceMode::Local;
                self.config.provider = InferenceProvider::LlamaCpp;
                self.config.runtime = "llama.cpp".into();
            }
        }
        self.config.initialized = true;
        self.config.status = "configured".into();
        self.save().await
    }

    pub async fn prepare_local_runtime(
        &self,
        entry: &ModelEntry,
        runtime_manager: &mut RuntimeManager,
    ) -> InferenceResult<()> {
        let mut adapter = LocalRuntimeAdapterBridge::new(runtime_manager.supervisor_mut());
        adapter.ensure_running().await?;
        if entry.file_path.exists() && self.config.provider == InferenceProvider::LlamaCpp {
            adapter.ensure_model_loaded(&entry.file_path).await?;
        }
        Ok(())
    }

    pub async fn build_provider_adapter(
        &self,
        entry: &ModelEntry,
        remote: Option<RemoteAdapterSettings>,
        model_provider: SharedModelProvider,
        runtime_manager: &mut RuntimeManager,
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
            InferenceMode::Local => {
                self.prepare_local_runtime(entry, runtime_manager).await?;
                let model_id = entry.id.clone();
                let provider_runtime =
                    ModelProviderRuntime::new(model_provider, model_id);
                Ok(Arc::new(LlamaCppAdapter::new(
                    self.config.provider,
                    entry.display_model_name(),
                    Arc::new(Mutex::new(provider_runtime)),
                )))
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
        let ModelSource::Remote {
            provider,
            model,
            base_url,
            region,
        } = &entry.source
        else {
            return Err(InferenceError::Config(
                "model is not a third-party remote entry".into(),
            ));
        };
        Ok(RemoteAdapterSettings {
            provider: InferenceProvider::parse(provider),
            model: model.clone(),
            base_url: base_url.clone(),
            api_key,
            aws_secret_access_key: aws_secret,
            aws_region: region.clone(),
            aws_session_token: aws_session,
        })
    }
}

pub fn config_path(data_dir: &Path) -> std::path::PathBuf {
    crate::config::config_path(data_dir)
}
