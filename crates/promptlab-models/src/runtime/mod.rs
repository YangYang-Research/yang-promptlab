mod inference_engine;
mod ollama;
mod mock;

use async_trait::async_trait;

pub use inference_engine::{
    infer_capabilities, infer_provider, infer_version, LocalInferenceEngine,
};
pub use ollama::{OllamaConfig, OllamaRuntime};
pub use mock::MockInferenceRuntime;

use crate::error::ModelResult;
use crate::types::{InferenceRequest, InferenceResponse, RuntimeState};

/// Local inference runtime contract (Ollama HTTP / mocks).
#[async_trait]
pub trait InferenceRuntime: Send + Sync {
    fn state(&self) -> RuntimeState;
    async fn load_model(&mut self, model_path: &std::path::Path) -> ModelResult<()>;
    async fn unload(&mut self) -> ModelResult<()>;
    async fn complete(&self, request: InferenceRequest) -> ModelResult<InferenceResponse>;
    async fn health(&self) -> ModelResult<bool>;
}
