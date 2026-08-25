use async_trait::async_trait;

use crate::capabilities::ModelCapabilities;
use crate::error::InferenceResult;
use crate::types::{ChatMessage, CompletionOutcome, ToolDefinition};

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

    /// LangChain-style tool calling. Default ignores tools and wraps [`Self::complete`].
    async fn complete_with_tools(
        &self,
        system: Option<&str>,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
        _tools: &[ToolDefinition],
        _tool_choice: Option<&serde_json::Value>,
    ) -> InferenceResult<CompletionOutcome> {
        let content = self
            .complete(system, prompt, max_tokens, temperature)
            .await?;
        Ok(CompletionOutcome::from_text(content))
    }

    /// Tool calling with a full OpenAI `messages[]` transcript.
    /// Default flattens to system + last user text and delegates to [`Self::complete_with_tools`].
    async fn complete_chat_with_tools(
        &self,
        messages: &[serde_json::Value],
        max_tokens: u32,
        temperature: f32,
        tools: &[ToolDefinition],
        tool_choice: Option<&serde_json::Value>,
    ) -> InferenceResult<CompletionOutcome> {
        let system = messages
            .iter()
            .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("system"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str());
        let prompt = messages
            .iter()
            .rev()
            .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("");
        self.complete_with_tools(system, prompt, max_tokens, temperature, tools, tool_choice)
            .await
    }

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
mod adapter_harness;
mod remote;
mod route;

pub use adapter_harness::AdapterHarness;
pub use remote::RemoteProviderAdapter;
pub use route::descriptor_from_remote;

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
