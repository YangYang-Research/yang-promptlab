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
    BrowserChat,
    McpServer,
}

impl TargetSurface {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().replace(['-', ' '], "_").as_str() {
            "rest_api" | "rest" | "http" | "api" => Some(Self::RestApi),
            "openai_compatible" | "openai" | "llm_api" => Some(Self::OpenAiCompatible),
            "anthropic_compatible" | "anthropic" => Some(Self::AnthropicCompatible),
            "browser_chat" | "chat_ui" | "browser" => Some(Self::BrowserChat),
            "mcp_server" | "mcp" => Some(Self::McpServer),
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
    Playwright,
}

impl HarnessKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::OpenAi => "openai",
            Self::Playwright => "playwright",
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

        Self {
            url,
            surface,
            method,
            headers,
            body_template,
            auth,
            browser_session_id,
            chat_selectors,
        }
    }

    pub fn preferred_harness(&self) -> HarnessKind {
        if self.browser_session_id.is_some() || self.surface == TargetSurface::BrowserChat {
            return HarnessKind::Playwright;
        }
        match self.surface {
            TargetSurface::OpenAiCompatible | TargetSurface::AnthropicCompatible => {
                HarnessKind::OpenAi
            }
            TargetSurface::BrowserChat => HarnessKind::Playwright,
            TargetSurface::RestApi | TargetSurface::McpServer => HarnessKind::Http,
        }
    }
}

fn infer_surface(value: &serde_json::Value) -> TargetSurface {
    if value
        .get("auth")
        .and_then(|a| a.get("engine"))
        .and_then(|v| v.as_str())
        == Some("playwright")
    {
        return TargetSurface::BrowserChat;
    }
    if value.get("type").and_then(|v| v.as_str()) == Some("llm_api") {
        return TargetSurface::OpenAiCompatible;
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
        "bearer" | "api_key" | "jwt" => {
            auth.bearer_token = value
                .get("token")
                .or_else(|| value.get("api_key"))
                .or_else(|| value.get("jwt"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
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
        assert_eq!(descriptor.preferred_harness(), HarnessKind::Playwright);
    }
}
