use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Built-in AI platform providers. Register new variants here for future platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetProvider {
    OpenAiCompatible,
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
}

impl TargetProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiCompatible => "openai_compatible",
            Self::AnthropicClaude => "anthropic_claude",
            Self::GoogleGemini => "google_gemini",
            Self::AzureOpenAi => "azure_openai",
            Self::AwsBedrock => "aws_bedrock",
            Self::GitHubCopilot => "github_copilot",
            Self::OpenWebUi => "open_webui",
            Self::Dify => "dify",
            Self::Langflow => "langflow",
            Self::Mcp => "mcp",
            Self::GenericHttp => "generic_http",
            Self::GenericWebSocket => "generic_websocket",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().replace(['-', ' '], "_").as_str() {
            "openai_compatible" | "openai" => Some(Self::OpenAiCompatible),
            "anthropic_claude" | "anthropic" | "claude" => Some(Self::AnthropicClaude),
            "google_gemini" | "gemini" => Some(Self::GoogleGemini),
            "azure_openai" | "azure" => Some(Self::AzureOpenAi),
            "aws_bedrock" | "bedrock" => Some(Self::AwsBedrock),
            "github_copilot" | "copilot" => Some(Self::GitHubCopilot),
            "open_webui" | "openwebui" => Some(Self::OpenWebUi),
            "dify" => Some(Self::Dify),
            "langflow" => Some(Self::Langflow),
            "mcp" | "mcp_server" => Some(Self::Mcp),
            "generic_http" | "generic" | "http" => Some(Self::GenericHttp),
            "generic_websocket" | "websocket" | "ws" => Some(Self::GenericWebSocket),
            _ => None,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::OpenAiCompatible => "OpenAI Compatible",
            Self::AnthropicClaude => "Anthropic Claude",
            Self::GoogleGemini => "Google Gemini",
            Self::AzureOpenAi => "Azure OpenAI",
            Self::AwsBedrock => "AWS Bedrock",
            Self::GitHubCopilot => "GitHub Copilot",
            Self::OpenWebUi => "Open WebUI",
            Self::Dify => "Dify",
            Self::Langflow => "Langflow",
            Self::Mcp => "MCP",
            Self::GenericHttp => "Generic HTTP API",
            Self::GenericWebSocket => "Generic WebSocket",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HttpMethod {
    #[default]
    Post,
    Get,
    Put,
    Patch,
    Delete,
}

impl HttpMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Post => "POST",
            Self::Get => "GET",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "POST" => Some(Self::Post),
            "GET" => Some(Self::Get),
            "PUT" => Some(Self::Put),
            "PATCH" => Some(Self::Patch),
            "DELETE" => Some(Self::Delete),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for HttpMethod {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        HttpMethod::parse(&raw).ok_or_else(|| serde::de::Error::custom("invalid HTTP method"))
    }
}

impl<'de> Deserialize<'de> for TargetProvider {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        TargetProvider::parse(&raw).ok_or_else(|| serde::de::Error::custom("invalid provider"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    #[default]
    None,
    ApiKey,
    BearerToken,
    OAuth,
    BasicAuth,
    Cookie,
    BrowserSession,
    CustomHeader,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TargetCapabilities {
    pub supports_streaming: bool,
    pub supports_tools: bool,
    pub supports_conversation: bool,
    pub supports_attachments: bool,
    pub supports_memory: bool,
    pub supports_agent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VerificationResult {
    pub verified: bool,
    #[serde(default, with = "crate::serde_verified_at")]
    pub verified_at: Option<OffsetDateTime>,
    pub provider: String,
    pub model: Option<String>,
    pub capabilities: TargetCapabilities,
    pub response_time_ms: u64,
    pub status_code: u16,
    pub status: String,
    pub response_preview: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetProfile {
    pub provider: TargetProvider,
    pub framework: String,
    pub method: HttpMethod,
    pub base_url: String,
    pub path: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub request_template: String,
    pub prompt_placeholder: String,
    pub model_field: Option<String>,
    pub streaming_field: Option<String>,
    pub conversation_field: Option<String>,
    pub tool_field: Option<String>,
    pub attachment_field: Option<String>,
    #[serde(default)]
    pub default_capabilities: TargetCapabilities,
    pub verification_strategy: String,
    #[serde(default)]
    pub verification: VerificationResult,
}

impl Default for TargetProfile {
    fn default() -> Self {
        let provider = TargetProvider::OpenAiCompatible;
        crate::templates::template_for_provider(provider)
    }
}

#[cfg(test)]
mod profile_serde_tests {
    use super::TargetProfile;

    #[test]
    fn deserializes_verification_verified_at_rfc3339() {
        let json = r#"{
          "provider": "generic_http",
          "framework": "openai",
          "method": "POST",
          "baseUrl": "https://api.example.com",
          "path": "/v1/chat",
          "requestTemplate": "{\"messages\":[{\"role\":\"user\",\"content\":\"{{PROMPT}}\"}]}",
          "promptPlaceholder": "{{PROMPT}}",
          "verificationStrategy": "generic_http",
          "verification": {
            "verified": true,
            "verifiedAt": "2025-06-13T12:00:00Z",
            "provider": "generic_http",
            "model": "test",
            "capabilities": {
              "supportsStreaming": false,
              "supportsTools": false,
              "supportsConversation": true,
              "supportsAttachments": false,
              "supportsMemory": false,
              "supportsAgent": false
            },
            "responseTimeMs": 100,
            "statusCode": 200,
            "status": "verified",
            "responsePreview": "hello",
            "errorMessage": null
          }
        }"#;
        let profile: TargetProfile = serde_json::from_str(json).expect("profile json");
        assert!(profile.verification.verified);
        assert!(profile.verification.verified_at.is_some());
    }

    #[test]
    fn deserializes_verification_verified_at_legacy_array() {
        use time::OffsetDateTime;

        let dt = OffsetDateTime::now_utc();
        let verified_at = serde_json::to_value(dt).expect("array");
        let json = serde_json::json!({
          "provider": "generic_http",
          "framework": "openai",
          "method": "POST",
          "baseUrl": "https://api.example.com",
          "path": "/v1/chat",
          "requestTemplate": "{\"messages\":[{\"role\":\"user\",\"content\":\"{{PROMPT}}\"}]}",
          "promptPlaceholder": "{{PROMPT}}",
          "verificationStrategy": "generic_http",
          "verification": {
            "verified": true,
            "verifiedAt": verified_at,
            "provider": "generic_http",
            "model": "test",
            "capabilities": {
              "supportsStreaming": false,
              "supportsTools": false,
              "supportsConversation": true,
              "supportsAttachments": false,
              "supportsMemory": false,
              "supportsAgent": false
            },
            "responseTimeMs": 100,
            "statusCode": 200,
            "status": "verified",
            "responsePreview": "hello",
            "errorMessage": null
          }
        });
        let profile: TargetProfile = serde_json::from_value(json).expect("profile json");
        assert!(profile.verification.verified_at.is_some());
    }
}

impl TargetProfile {
    pub fn full_url(&self) -> String {
        let base = self.base_url.trim_end_matches('/');
        let path = if self.path.starts_with('/') {
            self.path.clone()
        } else {
            format!("/{}", self.path)
        };
        format!("{base}{path}")
    }

    pub fn is_verified(&self) -> bool {
        self.verification.verified
    }
}
