use std::sync::Arc;

use async_trait::async_trait;
use promptlab_models::LocalModelManager;
use tokio::sync::Mutex;

use crate::error::RuntimeResult;
use crate::provider::{ModelProvider, ModelProviderHealth};

/// Bridges the model registry to the runtime [`ModelProvider`] contract.
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
    async fn complete_for_model(
        &self,
        model_id: &str,
        request: &promptlab_models::InferenceRequest,
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
                message: "no models registered".into(),
            });
        }

        let has_remote = manager.list_models().iter().any(|entry| {
            matches!(
                entry.provider,
                promptlab_models::ModelProvider::Remote
            )
        });
        Ok(ModelProviderHealth {
            healthy: has_remote,
            message: if has_remote {
                "vault has remote models configured".into()
            } else {
                "no remote models configured".into()
            },
        })
    }
}

pub type SharedModelProvider = Arc<dyn ModelProvider>;
