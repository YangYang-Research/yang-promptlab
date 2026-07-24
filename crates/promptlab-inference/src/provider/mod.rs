use async_trait::async_trait;

use crate::capabilities::ModelCapabilities;
use crate::error::InferenceResult;
use crate::types::ChatMessage;

/// Common provider adapter interface — only instantiated by RuntimeManager.
#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    fn provider_id(&self) -> &str;
    fn model_id(&self) -> &str;
    fn capabilities(&self) -> ModelCapabilities;

    async fn complete(
        &self,
        system: Option<&str>,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> InferenceResult<String>;

    async fn chat(
        &self,
        messages: &[ChatMessage],
        max_tokens: u32,
        temperature: f32,
    ) -> InferenceResult<String> {
        let system = messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.as_str());
        let user = messages
            .iter()
            .filter(|m| m.role == "user")
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        self.complete(system, &user, max_tokens, temperature)
            .await
    }

    async fn health(&self) -> InferenceResult<bool>;
}

pub mod bedrock_sigv4;
mod llama_cpp;
mod remote;

pub use llama_cpp::LlamaCppAdapter;
pub use remote::RemoteProviderAdapter;

pub use crate::config::InferenceProvider;

#[derive(Debug, Clone)]
pub struct RemoteAdapterSettings {
    pub provider: InferenceProvider,
    pub model: String,
    pub base_url: Option<String>,
    pub api_key: String,
    pub aws_secret_access_key: Option<String>,
    pub aws_region: Option<String>,
    pub aws_session_token: Option<String>,
}
