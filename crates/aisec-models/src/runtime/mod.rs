mod llama_cpp;
mod mock;

use async_trait::async_trait;

pub use llama_cpp::{LlamaCppConfig, LlamaCppRuntime};
pub use mock::MockInferenceRuntime;

use crate::error::ModelResult;
use crate::types::{InferenceRequest, InferenceResponse, RuntimeState};

/// llama.cpp inference runtime contract.
#[async_trait]
pub trait InferenceRuntime: Send + Sync {
    fn state(&self) -> RuntimeState;
    async fn load_model(&mut self, model_path: &std::path::Path) -> ModelResult<()>;
    async fn unload(&mut self) -> ModelResult<()>;
    async fn complete(&self, request: InferenceRequest) -> ModelResult<InferenceResponse>;
    async fn health(&self) -> ModelResult<bool>;
}
