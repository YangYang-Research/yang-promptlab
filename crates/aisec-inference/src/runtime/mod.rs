use async_trait::async_trait;

use crate::error::InferenceResult;

/// Abstracts embedded runtime lifecycle — RuntimeManager talks only to this trait.
#[async_trait]
pub trait RuntimeAdapter: Send + Sync {
    fn runtime_name(&self) -> &str;
    async fn ensure_running(&mut self) -> InferenceResult<()>;
    async fn ensure_model_loaded(&mut self, model_path: &std::path::Path) -> InferenceResult<()>;
    async fn health(&mut self) -> InferenceResult<bool>;
}

mod local;

pub use local::LocalRuntimeAdapterBridge;
