use async_trait::async_trait;
use promptlab_models::types::{InferenceRequest, InferenceResponse};
use serde::{Deserialize, Serialize};

use crate::error::RuntimeResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProviderHealth {
    pub healthy: bool,
    pub message: String,
}

/// Runtime provider contract for vault model inference (judge / local-LLM bridge).
#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn complete_for_model(
        &self,
        model_id: &str,
        request: &InferenceRequest,
    ) -> RuntimeResult<InferenceResponse>;
    async fn health(&self) -> RuntimeResult<ModelProviderHealth>;
}
