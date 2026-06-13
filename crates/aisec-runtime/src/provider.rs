use async_trait::async_trait;
use aisec_models::types::{InferenceRequest, InferenceResponse};
use serde::{Deserialize, Serialize};

use crate::error::RuntimeResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProviderHealth {
    pub healthy: bool,
    pub message: String,
}

/// Runtime provider contract for local model lifecycle.
#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn list_models(&self) -> RuntimeResult<Vec<String>>;
    async fn install_model(&self, model_id: &str) -> RuntimeResult<()>;
    async fn remove_model(&self, model_id: &str) -> RuntimeResult<()>;
    async fn run_inference(&self, model_id: &str, prompt: &str) -> RuntimeResult<String>;
    async fn complete_for_model(
        &self,
        model_id: &str,
        request: &InferenceRequest,
    ) -> RuntimeResult<InferenceResponse> {
        let text = self.run_inference(model_id, &request.prompt).await?;
        Ok(InferenceResponse {
            text,
            tokens_predicted: 0,
            duration_ms: 0,
        })
    }
    async fn health(&self) -> RuntimeResult<ModelProviderHealth>;
}
