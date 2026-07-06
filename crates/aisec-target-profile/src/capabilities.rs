use serde_json::Value;

use crate::types::{TargetCapabilities, TargetProfile, TargetProvider};

pub fn default_capabilities_for_provider(provider: TargetProvider) -> TargetCapabilities {
    match provider {
        TargetProvider::OpenAiCompatible
        | TargetProvider::OpenRouter
        | TargetProvider::AzureOpenAi
        | TargetProvider::GitHubCopilot
        | TargetProvider::OpenWebUi => TargetCapabilities {
            supports_streaming: true,
            supports_tools: true,
            supports_conversation: true,
            supports_attachments: true,
            supports_memory: false,
            supports_agent: false,
        },
        TargetProvider::AnthropicClaude | TargetProvider::AwsBedrock => TargetCapabilities {
            supports_streaming: true,
            supports_tools: true,
            supports_conversation: true,
            supports_attachments: true,
            supports_memory: false,
            supports_agent: false,
        },
        TargetProvider::GoogleGemini => TargetCapabilities {
            supports_streaming: true,
            supports_tools: true,
            supports_conversation: true,
            supports_attachments: true,
            supports_memory: false,
            supports_agent: false,
        },
        TargetProvider::Dify | TargetProvider::Langflow => TargetCapabilities {
            supports_streaming: true,
            supports_tools: false,
            supports_conversation: true,
            supports_attachments: false,
            supports_memory: true,
            supports_agent: true,
        },
        TargetProvider::Mcp => TargetCapabilities {
            supports_streaming: false,
            supports_tools: true,
            supports_conversation: false,
            supports_attachments: false,
            supports_memory: false,
            supports_agent: true,
        },
        TargetProvider::GenericHttp | TargetProvider::GenericWebSocket => TargetCapabilities {
            supports_streaming: false,
            supports_tools: false,
            supports_conversation: false,
            supports_attachments: false,
            supports_memory: false,
            supports_agent: false,
        },
    }
}

/// Merges stored profile capabilities with template heuristics for generic endpoints.
pub fn effective_capabilities(profile: &TargetProfile) -> TargetCapabilities {
    let mut caps = profile.default_capabilities.clone();
    if matches!(
        profile.provider,
        TargetProvider::GenericHttp | TargetProvider::GenericWebSocket
    ) {
        merge_capabilities(&mut caps, &infer_capabilities_from_template(profile));
    }
    caps
}

fn merge_capabilities(base: &mut TargetCapabilities, extra: &TargetCapabilities) {
    base.supports_streaming |= extra.supports_streaming;
    base.supports_tools |= extra.supports_tools;
    base.supports_conversation |= extra.supports_conversation;
    base.supports_attachments |= extra.supports_attachments;
    base.supports_memory |= extra.supports_memory;
    base.supports_agent |= extra.supports_agent;
}

pub(crate) fn merge_capabilities_into(base: &mut TargetCapabilities, extra: &TargetCapabilities) {
    merge_capabilities(base, extra);
}

fn infer_capabilities_from_template(profile: &TargetProfile) -> TargetCapabilities {
    let mut caps = TargetCapabilities::default();

    if profile.tool_field.as_deref().is_some_and(|s| !s.is_empty()) {
        caps.supports_tools = true;
    }
    if profile
        .conversation_field
        .as_deref()
        .is_some_and(|s| !s.is_empty())
    {
        caps.supports_conversation = true;
    }
    if profile
        .streaming_field
        .as_deref()
        .is_some_and(|s| !s.is_empty())
    {
        caps.supports_streaming = true;
    }
    if profile
        .attachment_field
        .as_deref()
        .is_some_and(|s| !s.is_empty())
    {
        caps.supports_attachments = true;
    }

    if let Ok(value) = serde_json::from_str::<Value>(&profile.request_template) {
        infer_capabilities_from_json_value(&value, &mut caps);
    }

    caps
}

fn infer_capabilities_from_json_value(value: &Value, caps: &mut TargetCapabilities) {
    let Some(obj) = value.as_object() else {
        return;
    };

    for (key, _) in obj {
        let key_lower = key.to_ascii_lowercase();
        if is_memory_key(&key_lower) {
            caps.supports_memory = true;
        }
        if is_agent_key(&key_lower) {
            caps.supports_agent = true;
        }
        if matches!(key_lower.as_str(), "tools" | "functions" | "tool_choice") {
            caps.supports_tools = true;
        }
        if matches!(
            key_lower.as_str(),
            "messages" | "contents" | "conversation" | "chat" | "history" | "input_value" | "query"
        ) {
            caps.supports_conversation = true;
        }
        if matches!(key_lower.as_str(), "stream" | "streaming" | "response_mode") {
            caps.supports_streaming = true;
        }
        if matches!(
            key_lower.as_str(),
            "attachments" | "files" | "parts" | "documents"
        ) {
            caps.supports_attachments = true;
        }
    }
}

fn is_memory_key(key: &str) -> bool {
    matches!(
        key,
        "chat_session_id"
            | "session_id"
            | "thread_id"
            | "conversation_id"
            | "memory"
            | "memory_id"
            | "user_memory"
            | "context_id"
    ) || key.contains("session")
        || key.contains("memory")
        || key.contains("thread")
}

fn is_agent_key(key: &str) -> bool {
    matches!(key, "agent_name" | "agent_id" | "agent" | "assistant_id" | "bot_id")
        || key.contains("agent")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn generic_http_infers_memory_and_agent_from_template() {
        let profile = TargetProfile {
            provider: TargetProvider::GenericHttp,
            framework: "openai".into(),
            request_template: r#"{
              "chat_session_id": "abc",
              "agent_name": "yang-code-review",
              "messages": [{ "role": "user", "content": "{{PROMPT}}" }]
            }"#
            .into(),
            conversation_field: Some("messages".into()),
            tool_field: Some("tools".into()),
            default_capabilities: TargetCapabilities {
                supports_streaming: true,
                supports_tools: true,
                supports_conversation: true,
                supports_attachments: true,
                supports_memory: false,
                supports_agent: false,
            },
            ..default_test_profile()
        };

        let caps = effective_capabilities(&profile);
        assert!(caps.supports_memory);
        assert!(caps.supports_agent);
        assert!(caps.supports_tools);
        assert!(caps.supports_conversation);
    }

    fn default_test_profile() -> TargetProfile {
        TargetProfile {
            method: crate::types::HttpMethod::Post,
            base_url: "https://api.example.com".into(),
            path: "/v1/chat".into(),
            headers: HashMap::new(),
            prompt_placeholder: "{{PROMPT}}".into(),
            model_field: None,
            streaming_field: None,
            conversation_field: None,
            tool_field: None,
            attachment_field: None,
            verification_strategy: "generic_http".into(),
            verification: Default::default(),
            ..TargetProfile::default()
        }
    }
}
