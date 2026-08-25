use std::sync::Arc;

use async_trait::async_trait;
use promptlab_models::{InferenceRequest, LocalModelManager};
use tokio::sync::Mutex;

use crate::error::RuntimeResult;
use crate::provider::{ModelProvider, ModelProviderHealth};

/// Bridges the embedded runtime supervisor to `promptlab-models` vault operations.
pub struct EmbeddedModelProvider {
    manager: Arc<Mutex<LocalModelManager>>,
}

impl EmbeddedModelProvider {
    pub fn new(manager: Arc<Mutex<LocalModelManager>>) -> Self {
        Self { manager }
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

    async fn install_model(&self, _model_id: &str) -> RuntimeResult<()> {
        Err(crate::error::RuntimeError::Model(
            "builtin GGUF catalog has been removed — add a remote third-party provider instead"
                .into(),
        ))
    }

    async fn remove_model(&self, model_id: &str) -> RuntimeResult<()> {
        let mut manager = self.manager.lock().await;
        manager.remove_model(model_id).await?;
        Ok(())
    }

    async fn run_inference(&self, model_id: &str, prompt: &str) -> RuntimeResult<String> {
        let response = self
            .complete_for_model(
                model_id,
                &InferenceRequest {
                    system: None,
                    prompt: prompt.to_string(),
                    max_tokens: 512,
                    temperature: 0.1,
                },
            )
            .await?;
        Ok(response.text)
    }

    async fn complete_for_model(
        &self,
        model_id: &str,
        request: &InferenceRequest,
    ) -> RuntimeResult<promptlab_models::types::InferenceResponse> {
        let manager = self.manager.lock().await;
        let engine = manager.inference_engine(model_id).await?;
        engine
            .complete(request.clone())
            .await
            .map_err(|err| crate::error::RuntimeError::Model(err.to_string()))
    }

    async fn health(&self) -> RuntimeResult<ModelProviderHealth> {
        let manager = self.manager.lock().await;
        if manager.list_models().is_empty() {
            return Ok(ModelProviderHealth {
                healthy: false,
                message: "no models installed in vault".into(),
            });
        }

        // Prefer remote/Ollama entries; embedded GGUF is no longer supported.
        let has_remote_or_ollama = manager.list_models().iter().any(|entry| {
            matches!(
                entry.provider,
                promptlab_models::ModelProvider::Remote | promptlab_models::ModelProvider::Ollama
            )
        });
        Ok(ModelProviderHealth {
            healthy: has_remote_or_ollama,
            message: if has_remote_or_ollama {
                "vault has remote or Ollama models configured".into()
            } else {
                "no remote/Ollama models configured — embedded GGUF runtime removed".into()
            },
        })
    }
}

pub type SharedModelProvider = Arc<dyn ModelProvider>;
