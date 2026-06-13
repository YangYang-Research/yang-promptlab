use async_trait::async_trait;

use crate::error::JudgeResult;

/// Unified LLM completion interface for local and remote judge backends.
#[async_trait]
pub trait LlmBackend: Send + Sync {
    fn provider_label(&self) -> &str;
    fn model_label(&self) -> &str;
    async fn complete(&self, prompt: &str, max_tokens: u32, temperature: f32) -> JudgeResult<String>;
    async fn health_check(&self) -> JudgeResult<bool> {
        Ok(true)
    }
}

pub mod local;
pub mod remote;

pub use local::LocalLlmBackend;
pub use remote::RemoteLlmBackend;
