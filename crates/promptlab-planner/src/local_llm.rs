use async_trait::async_trait;

use crate::error::PlannerResult;
use crate::tools::{LlmCompletion, ToolSpec};

/// LLM completion bridge for wizard attack-plan refinement and Yazg ReAct.
#[async_trait]
pub trait PlannerLlm: Send + Sync {
    async fn complete(&self, prompt: &str) -> PlannerResult<String>;

    /// LangChain-style tool calling: bind [`ToolSpec`] definitions and read
    /// [`LlmCompletion::tool_calls`]. Default falls back to text-only `complete`
    /// (no native tools) so callers can parse JSON-in-text when the provider
    /// does not support tool calling.
    async fn complete_with_tools(
        &self,
        prompt: &str,
        _tools: &[ToolSpec],
    ) -> PlannerResult<LlmCompletion> {
        let content = self.complete(prompt).await?;
        Ok(LlmCompletion::from_text(content))
    }
}
