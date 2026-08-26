mod inference_engine;
mod mock;

use async_trait::async_trait;

pub use inference_engine::{
    infer_capabilities, infer_provider, infer_version, LocalInferenceEngine,
};
pub use mock::MockInferenceRuntime;

use crate::error::ModelResult;
use crate::types::{InferenceRequest, InferenceResponse, RuntimeState};

/// Inference runtime contract (mocks / gateway bridges).
#[async_trait]
pub trait InferenceRuntime: Send + Sync {
    fn state(&self) -> RuntimeState;
    async fn complete(&self, request: InferenceRequest) -> ModelResult<InferenceResponse>;
    async fn health(&self) -> ModelResult<bool>;
}
