mod llama_cpp;
mod ollama;
mod inference_engine;
#[cfg(feature = "llama")]
mod llama_inproc;
mod mock;

use async_trait::async_trait;

pub use inference_engine::{
    infer_capabilities, infer_provider, infer_version, LocalInferenceEngine,
};
pub use llama_cpp::{LlamaCppConfig, LlamaCppRuntime};
pub use ollama::{OllamaConfig, OllamaRuntime};
#[cfg(feature = "llama")]
pub use llama_inproc::{LlamaInProcessRuntime, LlamaModelConfig};
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
