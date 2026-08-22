use async_trait::async_trait;
use promptlab_harness::{AttackRequest, ChatMessage, ChatTool, HarnessFactory, HarnessPurpose};
use serde_json::json;

use super::route::descriptor_from_remote;
use super::{ProviderAdapter, RemoteAdapterSettings};
use crate::capabilities::ModelCapabilities;
use crate::config::InferenceProvider;
use crate::error::{InferenceError, InferenceResult};
use crate::prompts::PromptRegistry;
use crate::types::{ChatMessage as InferenceChatMessage, CompletionOutcome, ToolCall, ToolDefinition};

pub struct RemoteProviderAdapter {
    settings: RemoteAdapterSettings,
    factory: HarnessFactory,
}

impl RemoteProviderAdapter {
    pub fn new(settings: RemoteAdapterSettings) -> Self {
        let factory = HarnessFactory::new().unwrap_or_else(|_| {
            HarnessFactory::from_registry(promptlab_harness::HarnessRegistry::new())
        });
        Self { settings, factory }
    }

    fn timeout_ms(&self) -> u64 {
        120_000
    }
}

#[async_trait]
impl ProviderAdapter for RemoteProviderAdapter {
    fn provider_id(&self) -> &str {
        self.settings.provider.as_str()
    }

    fn model_id(&self) -> &str {
        &self.settings.model
    }

    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities::from_remote(self.provider_id())
    }

    async fn complete(
        &self,
        system: Option<&str>,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> InferenceResult<String> {
        let outcome = self
            .complete_with_tools(system, prompt, max_tokens, temperature, &[], None)
            .await?;
        outcome.content.ok_or_else(|| {
            InferenceError::Provider("remote llm returned empty content".into())
        })
    }

    async fn complete_with_tools(
        &self,
        system: Option<&str>,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
        tools: &[ToolDefinition],
        tool_choice: Option<&serde_json::Value>,
    ) -> InferenceResult<CompletionOutcome> {
        let mut messages = Vec::new();
        if let Some(system) = system.map(str::trim).filter(|value| !value.is_empty()) {
            messages.push(json!({"role": "system", "content": system}));
        } else {
            messages.push(json!({
                "role": "system",
                "content": PromptRegistry::inference_system()
            }));
        }
        messages.push(json!({"role": "user", "content": prompt}));
        self.complete_chat_with_tools(&messages, max_tokens, temperature, tools, tool_choice)
            .await
    }

    async fn complete_chat_with_tools(
        &self,
        messages: &[serde_json::Value],
        max_tokens: u32,
        temperature: f32,
        tools: &[ToolDefinition],
        tool_choice: Option<&serde_json::Value>,
    ) -> InferenceResult<CompletionOutcome> {
        let descriptor = descriptor_from_remote(&self.settings);
        let chat_messages: Vec<ChatMessage> =
            messages.iter().map(ChatMessage::from_json).collect();
        let mut request = AttackRequest::from_chat(descriptor.url.clone(), chat_messages);
        request.purpose = HarnessPurpose::assistant();
        request.auth = descriptor.auth.clone();
        request.model = Some(self.settings.model.clone());
        request.max_tokens = Some(max_tokens);
        request.temperature = Some(temperature);
        request.timeout_ms = self.timeout_ms();
        request.headers = descriptor.headers.clone();
        request.tools = tools
            .iter()
            .map(|tool| ChatTool {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            })
            .collect();
        request.tool_choice = tool_choice.cloned();
        if self.settings.provider == InferenceProvider::OpenRouter
            || descriptor.url.to_ascii_lowercase().contains("openrouter.ai")
        {
            request
                .headers
                .insert("HTTP-Referer".into(), "https://promptlab.local".into());
            request.headers.insert("X-Title".into(), "PromptLab".into());
        }
        let response = self
            .factory
            .execute(&descriptor, request)
            .await
            .map_err(|err| InferenceError::Provider(err.to_string()))?;
        if response.content.trim().is_empty() && response.tool_calls.is_empty() {
            if matches!(self.settings.provider, InferenceProvider::OpenRouter)
                || descriptor.url.to_ascii_lowercase().contains("openrouter.ai")
            {
                if connectivity_response_has_completion(
                    &serde_json::from_str(&response.raw_response).unwrap_or(json!({})),
                ) {
                    return Ok(CompletionOutcome {
                        content: Some(response.content),
                        tool_calls: Vec::new(),
                        input_tokens: response.usage_input_tokens.unwrap_or(0),
                        output_tokens: response.usage_output_tokens.unwrap_or(0),
                    });
                }
            }
            return Err(InferenceError::Provider(
                "remote llm returned empty content".into(),
            ));
        }
        let input = response.usage_input_tokens.unwrap_or(0);
        let output = response.usage_output_tokens.unwrap_or(0);
        if input > 0 || output > 0 {
            crate::token_usage::record_completion(input, output);
        }
        Ok(CompletionOutcome {
            content: if response.content.trim().is_empty() {
                None
            } else {
                Some(response.content)
            },
            tool_calls: response
                .tool_calls
                .into_iter()
                .map(|call| ToolCall {
                    id: call.id.unwrap_or_else(|| format!("call_{}", call.name)),
                    name: call.name,
                    arguments: serde_json::from_str(&call.arguments)
                        .unwrap_or_else(|_| json!({})),
                })
                .collect(),
            input_tokens: input,
            output_tokens: output,
        })
    }

    async fn chat(
        &self,
        messages: &[InferenceChatMessage],
        max_tokens: u32,
        temperature: f32,
    ) -> InferenceResult<String> {
        let wire: Vec<serde_json::Value> = messages
            .iter()
            .map(|message| json!({"role": message.role, "content": message.content}))
            .collect();
        let outcome = self
            .complete_chat_with_tools(&wire, max_tokens, temperature, &[], None)
            .await?;
        outcome.content.ok_or_else(|| {
            InferenceError::Provider("remote llm returned empty content".into())
        })
    }

    async fn health(&self) -> InferenceResult<bool> {
        let sample = self
            .complete(
                Some(PromptRegistry::health_check_system()),
                PromptRegistry::health_check_user(),
                32,
                0.0,
            )
            .await?;
        Ok(!sample.trim().is_empty())
    }
}

fn connectivity_response_has_completion(value: &serde_json::Value) -> bool {
    let finish_ok = value
        .pointer("/choices/0/finish_reason")
        .and_then(|v| v.as_str())
        .is_some_and(|reason| reason == "stop" || reason == "length");
    let tokens = value
        .pointer("/usage/completion_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    finish_ok && tokens > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connectivity_response_has_completion_accepts_openrouter_usage() {
        let value = json!({
            "choices":[{"finish_reason":"stop","message":{"content":"hello"}}],
            "usage":{"completion_tokens":12}
        });
        assert!(connectivity_response_has_completion(&value));
    }
}