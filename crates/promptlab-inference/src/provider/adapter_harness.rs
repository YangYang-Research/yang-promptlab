use std::sync::Arc;

use async_trait::async_trait;
use promptlab_harness::{
    AttackRequest, Harness, HarnessResult, NormalizedResponse,
};

use super::ProviderAdapter;
use crate::types::ToolDefinition;

/// Registers a leftover [`ProviderAdapter`] on an isolated factory so tests still
/// go through `HarnessFactory::execute`.
pub struct AdapterHarness {
    adapter: Arc<dyn ProviderAdapter>,
}

impl AdapterHarness {
    pub fn new(adapter: Arc<dyn ProviderAdapter>) -> Self {
        Self { adapter }
    }
}

#[async_trait]
impl Harness for AdapterHarness {
    fn id(&self) -> &'static str {
        "llama"
    }

    async fn execute(&self, request: AttackRequest) -> HarnessResult<NormalizedResponse> {
        request.cancel.check()?;
        let messages = request
            .resolved_messages()
            .into_iter()
            .map(|message| {
                let mut value = serde_json::json!({
                    "role": message.role,
                    "content": message.content,
                });
                if let Some(name) = message.name {
                    value["name"] = serde_json::Value::String(name);
                }
                if let Some(tool_call_id) = message.tool_call_id {
                    value["tool_call_id"] = serde_json::Value::String(tool_call_id);
                }
                if let Some(tool_calls) = message.tool_calls {
                    value["tool_calls"] = tool_calls;
                }
                value
            })
            .collect::<Vec<_>>();
        let tools: Vec<ToolDefinition> = request
            .tools
            .iter()
            .map(|tool| ToolDefinition {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            })
            .collect();
        let max_tokens = request.max_tokens.unwrap_or(1024);
        let temperature = request.temperature.unwrap_or(0.0);
        let outcome = if messages.iter().any(|m| {
            m.get("role").and_then(|r| r.as_str()) == Some("tool")
                || m.get("tool_calls").is_some()
        }) || !tools.is_empty()
        {
            self.adapter
                .complete_chat_with_tools(
                    &messages,
                    max_tokens,
                    temperature,
                    &tools,
                    request.tool_choice.as_ref(),
                )
                .await
        } else {
            let (system, prompt) = request.system_and_user_prompt();
            self.adapter
                .complete_with_tools(
                    system.as_deref(),
                    &prompt,
                    max_tokens,
                    temperature,
                    &tools,
                    request.tool_choice.as_ref(),
                )
                .await
        }
        .map_err(|err| promptlab_harness::HarnessError::transport(err.to_string()))?;

        let mut response = NormalizedResponse {
            content: outcome.content.clone().unwrap_or_default(),
            raw_response: outcome.content.clone().unwrap_or_default(),
            usage_input_tokens: Some(outcome.input_tokens).filter(|v| *v > 0),
            usage_output_tokens: Some(outcome.output_tokens).filter(|v| *v > 0),
            tool_calls: outcome
                .tool_calls
                .into_iter()
                .map(|call| promptlab_harness::NormalizedToolCall {
                    id: Some(call.id),
                    name: call.name,
                    arguments: call.arguments.to_string(),
                })
                .collect(),
            ..NormalizedResponse::default()
        };
        response
            .metadata
            .insert("harness".into(), self.adapter.provider_id().into());
        if response.content.trim().is_empty() && response.tool_calls.is_empty() {
            response.error_class = Some("empty".into());
        }
        Ok(response)
    }
}
