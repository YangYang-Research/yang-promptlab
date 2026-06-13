use std::sync::Arc;

use async_trait::async_trait;
use aisec_models::{InferenceRequest, InferenceRuntime, LocalModelManager};
use tokio::sync::Mutex;

use crate::error::RuntimeResult;
use crate::provider::{ModelProvider, ModelProviderHealth};

/// Bridges the embedded runtime supervisor to `aisec-models` vault operations.
pub struct EmbeddedModelProvider {
    manager: Mutex<LocalModelManager>,
}

impl EmbeddedModelProvider {
    pub fn new(manager: LocalModelManager) -> Self {
        Self {
            manager: Mutex::new(manager),
        }
    }
}

#[async_trait]
impl ModelProvider for EmbeddedModelProvider {
    async fn list_models(&self) -> RuntimeResult<Vec<String>> {
        let manager = self.manager.lock().await;
        Ok(manager
            .list_models()
            .into_iter()
            .map(|entry| entry.id.clone())
            .collect())
    }

    async fn install_model(&self, model_id: &str) -> RuntimeResult<()> {
        let mut manager = self.manager.lock().await;
        manager.install_catalog(model_id, None).await?;
        Ok(())
    }

    async fn remove_model(&self, model_id: &str) -> RuntimeResult<()> {
        let mut manager = self.manager.lock().await;
        manager.remove_model(model_id).await?;
        Ok(())
    }

    async fn run_inference(&self, model_id: &str, prompt: &str) -> RuntimeResult<String> {
        let mut manager = self.manager.lock().await;
        manager.load(model_id).await?;
        let response = manager
            .complete(InferenceRequest {
                prompt: prompt.to_string(),
                max_tokens: 512,
                temperature: 0.1,
            })
            .await?;
        Ok(response.text)
    }

    async fn health(&self) -> RuntimeResult<ModelProviderHealth> {
        let manager = self.manager.lock().await;
        let healthy = manager.runtime().health().await.unwrap_or(false);
        Ok(ModelProviderHealth {
            healthy,
            message: if healthy {
                "local inference runtime is healthy".into()
            } else {
                "local inference runtime is unavailable".into()
            },
        })
    }
}

pub type SharedModelProvider = Arc<dyn ModelProvider>;
