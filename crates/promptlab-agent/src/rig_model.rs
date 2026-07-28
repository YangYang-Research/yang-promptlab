//! Rig `CompletionModel` adapter over PromptLab's `PlannerLlm` / AI Runtime.

use std::sync::Arc;

use promptlab_planner::{LlmCompletion, PlannerLlm, ToolSpec};
use rig::OneOrMany;
use rig::completion::{
    AssistantContent, CompletionError, CompletionModel, CompletionRequest, CompletionResponse,
    GetTokenUsage, Message, Usage,
};
use rig::message::{ToolCall, ToolFunction, UserContent};
use rig::streaming::StreamingCompletionResponse;
use serde::{Deserialize, Serialize};

/// Cloneable Rig model that delegates completions to PromptLab inference.
#[derive(Clone)]
pub struct YazgRigModel {
    llm: Arc<dyn PlannerLlm>,
}

impl YazgRigModel {
    pub fn new(llm: Arc<dyn PlannerLlm>) -> Self {
        Self { llm }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct YazgRigRawResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<promptlab_planner::ToolCall>,
}

impl GetTokenUsage for YazgRigRawResponse {
    fn token_usage(&self) -> Usage {
        Usage::new()
    }
}

impl CompletionModel for YazgRigModel {
    type Response = YazgRigRawResponse;
    type StreamingResponse = YazgRigRawResponse;
    type Client = ();

    fn make(_: &Self::Client, _: impl Into<String>) -> Self {
        // Placeholder; production paths always construct via `new`.
        Self {
            llm: Arc::new(UnsupportedLlm),
        }
    }

    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<Self::Response>, CompletionError> {
        let prompt = flatten_completion_request(&request);
        let tools = request
            .tools
            .iter()
            .map(|tool| ToolSpec::new(&tool.name, &tool.description, tool.parameters.clone()))
            .collect::<Vec<_>>();

        let outcome = if tools.is_empty() {
            let content = self
                .llm
                .complete(&prompt)
                .await
                .map_err(|err| CompletionError::ProviderError(err.to_string()))?;
            LlmCompletion::from_text(content)
        } else {
            self.llm
                .complete_with_tools(&prompt, &tools)
                .await
                .map_err(|err| CompletionError::ProviderError(err.to_string()))?
        };

        let raw = YazgRigRawResponse {
            content: outcome.content.clone(),
            tool_calls: outcome.tool_calls.clone(),
        };

        let choice = completion_to_assistant_content(&outcome)?;
        Ok(CompletionResponse {
            choice,
            usage: Usage::new(),
            raw_response: raw,
            message_id: None,
        })
    }

    async fn stream(
        &self,
        _request: CompletionRequest,
    ) -> Result<StreamingCompletionResponse<Self::StreamingResponse>, CompletionError> {
        Err(CompletionError::ProviderError(
            "YazgRigModel does not support streaming yet".into(),
        ))
    }
}

struct UnsupportedLlm;

#[async_trait::async_trait]
impl PlannerLlm for UnsupportedLlm {
    async fn complete(&self, _prompt: &str) -> promptlab_planner::PlannerResult<String> {
        Err(promptlab_planner::PlannerError::Llm(
            "YazgRigModel placeholder has no LLM bound".into(),
        ))
    }
}

fn completion_to_assistant_content(
    outcome: &LlmCompletion,
) -> Result<OneOrMany<AssistantContent>, CompletionError> {
    let mut parts = Vec::new();
    if let Some(content) = outcome
        .content
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        parts.push(AssistantContent::text(content));
    }
    for call in &outcome.tool_calls {
        parts.push(AssistantContent::ToolCall(ToolCall::new(
            call.id.clone(),
            ToolFunction::new(call.name.clone(), call.arguments.clone()),
        )));
    }
    match parts.len() {
        0 => Err(CompletionError::ResponseError(
            "model returned empty content and no tool calls".into(),
        )),
        1 => Ok(OneOrMany::one(parts.remove(0))),
        _ => OneOrMany::many(parts).map_err(|_| {
            CompletionError::ResponseError("failed to build assistant content".into())
        }),
    }
}

fn flatten_completion_request(request: &CompletionRequest) -> String {
    let mut out = String::new();
    if let Some(preamble) = request.preamble.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        out.push_str("System:\n");
        out.push_str(preamble);
        out.push_str("\n\n");
    }
    for message in request.chat_history.iter() {
        match message {
            Message::System { content } => {
                out.push_str("System:\n");
                out.push_str(content);
                out.push_str("\n\n");
            }
            Message::User { content } => {
                out.push_str("User:\n");
                out.push_str(&flatten_user_content(content));
                out.push_str("\n\n");
            }
            Message::Assistant { content, .. } => {
                out.push_str("Assistant:\n");
                out.push_str(&flatten_assistant_content(content));
                out.push_str("\n\n");
            }
        }
    }
    for doc in &request.documents {
        out.push_str("Context:\n");
        out.push_str(&doc.to_string());
        out.push('\n');
    }
    out.trim().to_string()
}

fn flatten_user_content(content: &OneOrMany<UserContent>) -> String {
    let mut parts = Vec::new();
    for item in content.iter() {
        match item {
            UserContent::Text(text) => parts.push(text.text.clone()),
            UserContent::ToolResult(result) => {
                let body = result
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        rig::message::ToolResultContent::Text(t) => Some(t.text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                parts.push(format!("[tool_result id={}]\n{body}", result.id));
            }
            other => parts.push(format!("{other:?}")),
        }
    }
    parts.join("\n")
}

fn flatten_assistant_content(content: &OneOrMany<AssistantContent>) -> String {
    let mut parts = Vec::new();
    for item in content.iter() {
        match item {
            AssistantContent::Text(text) => parts.push(text.text.clone()),
            AssistantContent::ToolCall(call) => parts.push(format!(
                "[tool_call id={} name={} args={}]",
                call.id, call.function.name, call.function.arguments
            )),
            AssistantContent::Reasoning(reasoning) => {
                parts.push(format!("[reasoning] {reasoning:?}"));
            }
            AssistantContent::Image(_) => parts.push("[image]".into()),
        }
    }
    parts.join("\n")
}
