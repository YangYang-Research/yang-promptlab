use std::collections::HashMap;

use crate::capabilities::default_capabilities_for_provider;
use crate::prompt::PROMPT_PLACEHOLDER;
use crate::types::{HttpMethod, TargetCapabilities, TargetProfile, TargetProvider};

fn profile(
    provider: TargetProvider,
    framework: &str,
    base_url: &str,
    path: &str,
    headers: HashMap<&str, &str>,
    request_template: &str,
    model_field: Option<&str>,
    streaming_field: Option<&str>,
    conversation_field: Option<&str>,
    tool_field: Option<&str>,
    attachment_field: Option<&str>,
    verification_strategy: &str,
) -> TargetProfile {
    TargetProfile {
        provider,
        framework: framework.into(),
        method: HttpMethod::Post,
        base_url: base_url.into(),
        path: path.into(),
        headers: headers
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        request_template: request_template.into(),
        prompt_placeholder: PROMPT_PLACEHOLDER.into(),
        model_field: model_field.map(str::to_string),
        streaming_field: streaming_field.map(str::to_string),
        conversation_field: conversation_field.map(str::to_string),
        tool_field: tool_field.map(str::to_string),
        attachment_field: attachment_field.map(str::to_string),
        default_capabilities: default_capabilities_for_provider(provider),
        verification_strategy: verification_strategy.into(),
        verification: Default::default(),
    }
}

pub fn template_for_provider(provider: TargetProvider) -> TargetProfile {
    match provider {
        TargetProvider::OpenAiCompatible => profile(
            provider,
            "openai",
            "https://api.openai.com",
            "/v1/chat/completions",
            HashMap::from([("Content-Type", "application/json")]),
            r#"{
  "model": "gpt-4o-mini",
  "messages": [
    { "role": "user", "content": "{{PROMPT}}" }
  ],
  "stream": false
}"#,
            Some("model"),
            Some("stream"),
            Some("messages"),
            Some("tools"),
            None,
            "openai_chat_completion",
        ),
        TargetProvider::OpenRouter => profile(
            provider,
            "openrouter",
            "https://openrouter.ai/api/v1",
            "/chat/completions",
            HashMap::from([("Content-Type", "application/json")]),
            r#"{
  "model": "google/gemini-2.5-flash-lite",
  "messages": [
    { "role": "user", "content": "{{PROMPT}}" }
  ],
  "stream": false
}"#,
            Some("model"),
            Some("stream"),
            Some("messages"),
            Some("tools"),
            None,
            "openai_chat_completion",
        ),
        TargetProvider::AnthropicClaude => profile(
            provider,
            "anthropic",
            "https://api.anthropic.com",
            "/v1/messages",
            HashMap::from([
                ("Content-Type", "application/json"),
                ("anthropic-version", "2023-06-01"),
            ]),
            r#"{
  "model": "claude-3-5-sonnet-20241022",
  "max_tokens": 256,
  "messages": [
    { "role": "user", "content": "{{PROMPT}}" }
  ]
}"#,
            Some("model"),
            None,
            Some("messages"),
            Some("tools"),
            None,
            "anthropic_messages",
        ),
        TargetProvider::GoogleGemini => profile(
            provider,
            "gemini",
            "https://generativelanguage.googleapis.com",
            "/v1beta/models/gemini-1.5-flash:generateContent",
            HashMap::from([("Content-Type", "application/json")]),
            r#"{
  "contents": [
    {
      "parts": [
        { "text": "{{PROMPT}}" }
      ]
    }
  ]
}"#,
            None,
            None,
            Some("contents"),
            None,
            Some("parts"),
            "gemini_generate_content",
        ),
        TargetProvider::AzureOpenAi => profile(
            provider,
            "azure_openai",
            "https://YOUR-RESOURCE.openai.azure.com",
            "/openai/deployments/YOUR-DEPLOYMENT/chat/completions?api-version=2024-02-15-preview",
            HashMap::from([("Content-Type", "application/json")]),
            r#"{
  "messages": [
    { "role": "user", "content": "{{PROMPT}}" }
  ],
  "stream": false
}"#,
            None,
            Some("stream"),
            Some("messages"),
            Some("tools"),
            None,
            "azure_openai_chat_completion",
        ),
        TargetProvider::AwsBedrock => profile(
            provider,
            "bedrock",
            "https://bedrock-runtime.us-east-1.amazonaws.com",
            "/model/anthropic.claude-3-sonnet-20240229-v1:0/invoke",
            HashMap::from([("Content-Type", "application/json")]),
            r#"{
  "anthropic_version": "bedrock-2023-05-31",
  "max_tokens": 256,
  "messages": [
    { "role": "user", "content": "{{PROMPT}}" }
  ]
}"#,
            None,
            None,
            Some("messages"),
            Some("tools"),
            None,
            "bedrock_invoke",
        ),
        TargetProvider::GitHubCopilot => profile(
            provider,
            "copilot",
            "https://api.githubcopilot.com",
            "/chat/completions",
            HashMap::from([("Content-Type", "application/json")]),
            r#"{
  "model": "gpt-4o",
  "messages": [
    { "role": "user", "content": "{{PROMPT}}" }
  ],
  "stream": false
}"#,
            Some("model"),
            Some("stream"),
            Some("messages"),
            None,
            None,
            "copilot_chat_completion",
        ),
        TargetProvider::OpenWebUi => profile(
            provider,
            "open_webui",
            "http://localhost:3000",
            "/api/chat/completions",
            HashMap::from([("Content-Type", "application/json")]),
            r#"{
  "model": "llama3",
  "messages": [
    { "role": "user", "content": "{{PROMPT}}" }
  ],
  "stream": false
}"#,
            Some("model"),
            Some("stream"),
            Some("messages"),
            Some("tools"),
            None,
            "open_webui_chat_completion",
        ),
        TargetProvider::Dify => profile(
            provider,
            "dify",
            "https://api.dify.ai",
            "/v1/chat-messages",
            HashMap::from([("Content-Type", "application/json")]),
            r#"{
  "query": "{{PROMPT}}",
  "response_mode": "blocking",
  "user": "promptlab-probe"
}"#,
            None,
            Some("response_mode"),
            Some("conversation_id"),
            None,
            None,
            "dify_chat_message",
        ),
        TargetProvider::Langflow => profile(
            provider,
            "langflow",
            "http://localhost:7860",
            "/api/v1/run/FLOW_ID",
            HashMap::from([("Content-Type", "application/json")]),
            r#"{
  "input_value": "{{PROMPT}}",
  "output_type": "chat",
  "input_type": "chat"
}"#,
            None,
            None,
            Some("session_id"),
            None,
            None,
            "langflow_run",
        ),
        TargetProvider::Mcp => profile(
            provider,
            "mcp",
            "http://localhost:8080",
            "/mcp",
            HashMap::from([("Content-Type", "application/json")]),
            r#"{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "chat",
    "arguments": { "prompt": "{{PROMPT}}" }
  }
}"#,
            None,
            None,
            None,
            Some("params"),
            None,
            "mcp_tools_call",
        ),
        TargetProvider::GenericHttp => TargetProfile {
            provider,
            framework: "generic".into(),
            method: HttpMethod::Post,
            base_url: "https://".into(),
            path: "/".into(),
            headers: HashMap::from([("Content-Type".into(), "application/json".into())]),
            request_template: format!(r#"{{ "prompt": "{PROMPT_PLACEHOLDER}" }}"#),
            prompt_placeholder: PROMPT_PLACEHOLDER.into(),
            model_field: None,
            streaming_field: None,
            conversation_field: None,
            tool_field: None,
            attachment_field: None,
            default_capabilities: default_capabilities_for_provider(provider),
            verification_strategy: "generic_http".into(),
            verification: Default::default(),
        },
        TargetProvider::GenericWebSocket => TargetProfile {
            provider,
            framework: "generic".into(),
            method: HttpMethod::Post,
            base_url: "wss://".into(),
            path: "/".into(),
            headers: HashMap::new(),
            request_template: format!(r#"{{ "message": "{PROMPT_PLACEHOLDER}" }}"#),
            prompt_placeholder: PROMPT_PLACEHOLDER.into(),
            model_field: None,
            streaming_field: None,
            conversation_field: None,
            tool_field: None,
            attachment_field: None,
            default_capabilities: default_capabilities_for_provider(provider),
            verification_strategy: "generic_websocket".into(),
            verification: Default::default(),
        },
    }
}

pub fn list_provider_templates() -> Vec<TargetProfile> {
    use TargetProvider::*;
    [
        OpenAiCompatible,
        OpenRouter,
        AnthropicClaude,
        GoogleGemini,
        AzureOpenAi,
        AwsBedrock,
        GitHubCopilot,
        OpenWebUi,
        Dify,
        Langflow,
        Mcp,
        GenericHttp,
        GenericWebSocket,
    ]
    .into_iter()
    .map(template_for_provider)
    .collect()
}
