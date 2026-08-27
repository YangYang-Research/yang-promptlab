use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{AuthMaterial, HttpMethod};

/// Target surface kind — determines harness selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TargetSurface {
    #[default]
    RestApi,
    OpenAiCompatible,
    AnthropicCompatible,
    Gemini,
    Dify,
    BrowserChat,
    McpServer,
    WebSocket,
    Bedrock,
    LlamaCpp,
    Ollama,
    LocalRuntime,
}

impl TargetSurface {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().replace(['-', ' '], "_").as_str() {
            "rest_api" | "rest" | "http" | "api" => Some(Self::RestApi),
            "openai_compatible" | "openai" | "llm_api" => Some(Self::OpenAiCompatible),
            "anthropic_compatible" | "anthropic" => Some(Self::AnthropicCompatible),
            "gemini" | "google_gemini" => Some(Self::Gemini),
            "dify" => Some(Self::Dify),
            "browser_chat" | "chat_ui" | "browser" => Some(Self::BrowserChat),
            "mcp_server" | "mcp" => Some(Self::McpServer),
            "websocket" | "generic_websocket" | "ws" => Some(Self::WebSocket),
            "bedrock" | "aws_bedrock" => Some(Self::Bedrock),
            "llama_cpp" | "llama.cpp" | "llama" => Some(Self::LlamaCpp),
            "ollama" => Some(Self::Ollama),
            "local_runtime" | "callback" | "adapter" => Some(Self::LocalRuntime),
            _ => None,
        }
    }
}

/// Harness implementation kind resolved by the factory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessKind {
    Http,
    OpenAi,
    Anthropic,
    Gemini,
    Dify,
    Mcp,
    WebSocket,
    Bedrock,
    Playwright,
    Llama,
}

impl HarnessKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
            Self::Dify => "dify",
            Self::Mcp => "mcp",
            Self::WebSocket => "websocket",
            Self::Bedrock => "bedrock",
            Self::Playwright => "playwright",
            Self::Llama => "llama",
        }
    }
}

/// Parsed target descriptor used by harness factory resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetDescriptor {
    pub url: String,
    #[serde(default)]
    pub surface: TargetSurface,
    #[serde(default)]
    pub method: HttpMethod,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub body_template: Option<String>,
    #[serde(default)]
    pub auth: AuthMaterial,
    pub browser_session_id: Option<String>,
    #[serde(default)]
    pub chat_selectors: HashMap<String, String>,
    #[serde(default)]
    pub stream: bool,
    pub conversation_id: Option<String>,
    pub mcp_method: Option<String>,
    pub mcp_session_id: Option<String>,
    pub ws_subprotocol: Option<String>,
}

impl Default for TargetDescriptor {
    fn default() -> Self {
        Self {
            url: String::new(),
            surface: TargetSurface::RestApi,
            method: HttpMethod::Post,
            headers: HashMap::new(),
            body_template: None,
            auth: AuthMaterial::default(),
            browser_session_id: None,
            chat_selectors: HashMap::new(),
            stream: false,
            conversation_id: None,
            mcp_method: None,
            mcp_session_id: None,
            ws_subprotocol: None,
        }
    }
}

impl TargetDescriptor {
    pub fn from_descriptor_json(json: &str) -> Result<Self, serde_json::Error> {
        let value: serde_json::Value = serde_json::from_str(json)?;
        Ok(Self::from_json_value(&value))
    }

    pub fn from_json_value(value: &serde_json::Value) -> Self {
        let url = value
            .get("url")
            .or_else(|| value.get("base_url"))
            .or_else(|| value.get("baseUrl"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let surface = value
            .get("surface")
            .or_else(|| value.get("target_type"))
            .and_then(|v| v.as_str())
            .and_then(TargetSurface::parse)
            .unwrap_or_else(|| infer_surface(value));

        let method = value
            .get("method")
            .and_then(|v| v.as_str())
            .and_then(HttpMethod::parse)
            .unwrap_or(HttpMethod::Post);

        let mut headers = HashMap::new();
        if let Some(map) = value.get("headers").and_then(|v| v.as_object()) {
            for (key, val) in map {
                if let Some(text) = val.as_str() {
                    headers.insert(key.clone(), text.to_string());
                }
            }
        }

        let body_template = value
            .get("body_template")
            .or_else(|| value.get("bodyTemplate"))
            .and_then(|v| v.as_str())
            .map(str::to_string);

        let mut auth = AuthMaterial::default();
        if let Some(auth_value) = value.get("auth") {
            parse_auth(auth_value, &mut auth);
        }

        let browser_session_id = value
            .get("auth")
            .and_then(|auth| auth.get("session_id"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string);

        let mut chat_selectors = HashMap::new();
        if let Some(selectors) = value.get("chat_selectors").and_then(|v| v.as_object()) {
            for (key, val) in selectors {
                if let Some(text) = val.as_str() {
                    chat_selectors.insert(key.clone(), text.to_string());
                }
            }
        }

        let stream = value
            .get("stream")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let conversation_id = value
            .get("conversation_id")
            .or_else(|| value.get("conversationId"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let mcp_method = value
            .get("mcp_method")
            .or_else(|| value.get("mcpMethod"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let mcp_session_id = value
            .get("mcp_session_id")
            .or_else(|| value.get("mcpSessionId"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let ws_subprotocol = value
            .get("ws_subprotocol")
            .or_else(|| value.get("wsSubprotocol"))
            .and_then(|v| v.as_str())
            .map(str::to_string);

        Self {
            url,
            surface,
            method,
            headers,
            body_template,
            auth,
            browser_session_id,
            chat_selectors,
            stream,
            conversation_id,
            mcp_method,
            mcp_session_id,
            ws_subprotocol,
        }
    }

    pub fn preferred_harness(&self) -> HarnessKind {
        #[cfg(feature = "playwright")]
        if self.browser_session_id.is_some() || self.surface == TargetSurface::BrowserChat {
            return HarnessKind::Playwright;
        }
        match self.surface {
            TargetSurface::OpenAiCompatible => HarnessKind::OpenAi,
            TargetSurface::AnthropicCompatible => HarnessKind::Anthropic,
            TargetSurface::Gemini => HarnessKind::Gemini,
            TargetSurface::Dify => HarnessKind::Dify,
            TargetSurface::BrowserChat => {
                #[cfg(feature = "playwright")]
                {
                    HarnessKind::Playwright
                }
                #[cfg(not(feature = "playwright"))]
                {
                    HarnessKind::Http
                }
            }
            TargetSurface::McpServer => HarnessKind::Mcp,
            TargetSurface::WebSocket => HarnessKind::WebSocket,
            TargetSurface::Bedrock => HarnessKind::Bedrock,
            TargetSurface::LlamaCpp | TargetSurface::Ollama | TargetSurface::LocalRuntime => {
                HarnessKind::Llama
            }
            TargetSurface::RestApi => HarnessKind::Http,
        }
    }
}

fn infer_surface(value: &serde_json::Value) -> TargetSurface {
    if let Some(kind) = value.get("type").and_then(|v| v.as_str()) {
        if let Some(parsed) = TargetSurface::parse(kind) {
            return parsed;
        }
        if kind == "llm_api" {
            return TargetSurface::OpenAiCompatible;
        }
    }
    TargetSurface::RestApi
}

fn parse_auth(value: &serde_json::Value, auth: &mut AuthMaterial) {
    let method = value
        .get("method")
        .or_else(|| value.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("none");

    match method {
        "basic" | "basic_auth" => {
            auth.basic_username = value
                .get("username")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            auth.basic_password = value
                .get("password")
                .and_then(|v| v.as_str())
                .map(str::to_string);
        }
        "bearer" | "jwt" => {
            auth.bearer_token = value
                .get("token")
                .or_else(|| value.get("api_key"))
                .or_else(|| value.get("jwt"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
        }
        "api_key" | "anthropic" | "gemini" => {
            auth.api_key = value
                .get("api_key")
                .or_else(|| value.get("token"))
                .or_else(|| value.get("key"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            auth.api_key_header = value
                .get("header")
                .or_else(|| value.get("header_name"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .or_else(|| match method {
                    "anthropic" => Some("x-api-key".into()),
                    "gemini" => Some("x-goog-api-key".into()),
                    _ => Some("x-api-key".into()),
                });
            if method == "gemini" {
                auth.query_key_name = Some("key".into());
                auth.query_key_value = auth.api_key.clone();
            }
        }
        "query_key" => {
            auth.query_key_name = value
                .get("name")
                .or_else(|| value.get("query_name"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .or(Some("key".into()));
            auth.query_key_value = value
                .get("value")
                .or_else(|| value.get("api_key"))
                .or_else(|| value.get("token"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
        }
        "aws" | "sigv4" | "bedrock" => {
            auth.aws_access_key_id = value
                .get("access_key_id")
                .or_else(|| value.get("accessKeyId"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            auth.aws_secret_access_key = value
                .get("secret_access_key")
                .or_else(|| value.get("secretAccessKey"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            auth.aws_session_token = value
                .get("session_token")
                .or_else(|| value.get("sessionToken"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            auth.aws_region = value
                .get("region")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            auth.aws_service = value
                .get("service")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .or(Some("bedrock".into()));
        }
        "playwright" | "browser_session" => {
            if let Some(path) = value.get("storage_state_path").and_then(|v| v.as_str()) {
                auth.storage_state_path = Some(path.to_string());
            }
        }
        _ => {}
    }

    if let Some(headers) = value.get("headers").and_then(|v| v.as_object()) {
        for (key, val) in headers {
            if let Some(text) = val.as_str() {
                auth.headers.insert(key.clone(), text.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_playwright_descriptor() {
        let json = r#"{
            "url": "https://app.example/chat",
            "auth": { "engine": "playwright", "session_id": "sess-1" }
        }"#;
        let descriptor = TargetDescriptor::from_descriptor_json(json).unwrap();
        assert_eq!(descriptor.browser_session_id.as_deref(), Some("sess-1"));
        #[cfg(feature = "playwright")]
        assert_eq!(descriptor.preferred_harness(), HarnessKind::Playwright);
        #[cfg(not(feature = "playwright"))]
        assert_eq!(descriptor.preferred_harness(), HarnessKind::Http);
    }

    #[test]
    fn maps_anthropic_surface_to_anthropic_harness() {
        let json = r#"{"url":"https://api.anthropic.com/v1/messages","surface":"anthropic_compatible"}"#;
        let descriptor = TargetDescriptor::from_descriptor_json(json).unwrap();
        assert_eq!(descriptor.preferred_harness(), HarnessKind::Anthropic);
    }

    #[test]
    fn maps_mcp_and_websocket_surfaces() {
        let mcp = TargetDescriptor {
            surface: TargetSurface::McpServer,
            ..TargetDescriptor::default()
        };
        assert_eq!(mcp.preferred_harness(), HarnessKind::Mcp);
        let ws = TargetDescriptor {
            surface: TargetSurface::WebSocket,
            ..TargetDescriptor::default()
        };
        assert_eq!(ws.preferred_harness(), HarnessKind::WebSocket);
    }
}
