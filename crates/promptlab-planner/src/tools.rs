//! LangChain-style tool calling types for PlannerLlm.
//!
//! Models that support native tool calling receive [`ToolSpec`] definitions
//! (name + description + JSON Schema parameters) and may return [`ToolCall`]s
//! instead of free-form text. See:
//! <https://www.langchain.com/blog/tool-calling-with-langchain>

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A tool the model may invoke (bound via `complete_with_tools`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema object for tool arguments.
    pub parameters: Value,
}

impl ToolSpec {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

/// One tool invocation chosen by the model (`AIMessage.tool_calls` equivalent).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// Structured completion after a tool-aware model call.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LlmCompletion {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
}

impl LlmCompletion {
    pub fn from_text(content: impl Into<String>) -> Self {
        Self {
            content: Some(content.into()),
            tool_calls: Vec::new(),
        }
    }

    pub fn primary_tool(&self) -> Option<&ToolCall> {
        self.tool_calls.first()
    }
}
