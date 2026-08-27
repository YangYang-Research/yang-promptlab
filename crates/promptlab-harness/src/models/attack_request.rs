use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use crate::cancel::CancelFlag;

use super::chat::{ChatMessage, ChatTool, StreamChunk};

/// Why this I/O happened. New app surfaces add a name — no crate bump required.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HarnessPurpose(String);

impl HarnessPurpose {
    pub fn attack() -> Self {
        Self("attack".into())
    }
    pub fn verify() -> Self {
        Self("verify".into())
    }
    pub fn discover() -> Self {
        Self("discover".into())
    }
    pub fn fingerprint() -> Self {
        Self("fingerprint".into())
    }
    pub fn assistant() -> Self {
        Self("assistant".into())
    }
    pub fn judge() -> Self {
        Self("judge".into())
    }
    pub fn wizard() -> Self {
        Self("wizard".into())
    }
    pub fn planner() -> Self {
        Self("planner".into())
    }
    pub fn generator() -> Self {
        Self("generator".into())
    }
    pub fn report() -> Self {
        Self("report".into())
    }
    pub fn health() -> Self {
        Self("health".into())
    }
    pub fn named(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Map a token-meter / host agent id onto a harness purpose.
    pub fn from_agent_id(agent_id: &str) -> Self {
        let key = agent_id.trim().to_ascii_lowercase();
        match key.as_str() {
            "yazg" | "assistant" => Self::assistant(),
            "judge" | "judge_worker" | "classifier_worker" | "attacker_worker" => Self::judge(),
            "wizard" | "analyze_endpoint" | "endpoint_verify" => Self::wizard(),
            "planner" | "attack_plan" => Self::planner(),
            "generator" | "generate_prompt" => Self::generator(),
            "report" | "recommend" | "summary" => Self::report(),
            "discover" => Self::discover(),
            "verify" => Self::verify(),
            "fingerprint" => Self::fingerprint(),
            "health" | "test_chat" | "test_connectivity" | "test_inference" => Self::health(),
            "attack" => Self::attack(),
            "agentic_attack_execution"
            | "sequential_attack_execution"
            | "reflection"
            | "judge_coordinator" => Self::planner(),
            other if other.is_empty() => Self::assistant(),
            other => Self::named(other),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn is_attack(&self) -> bool {
        self.0 == "attack"
    }
    pub fn is_product_inference(&self) -> bool {
        matches!(
            self.0.as_str(),
            "assistant"
                | "judge"
                | "wizard"
                | "planner"
                | "generator"
                | "report"
                | "health"
                | "test_chat"
        )
    }

    /// Target-facing probes keep HTTP error status as observations (recover/judge).
    /// Product inference must not treat HTTP errors, 429, timeout, or empty bodies as completions.
    pub fn fails_on_retryable_http(&self) -> bool {
        !matches!(
            self.0.as_str(),
            "attack" | "verify" | "discover" | "fingerprint"
        )
    }
}

impl Default for HarnessPurpose {
    fn default() -> Self {
        Self::attack()
    }
}

/// HTTP verb for harness delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl HttpMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }

    pub fn parse(method: &str) -> Option<Self> {
        match method.trim().to_uppercase().as_str() {
            "GET" => Some(Self::Get),
            "POST" => Some(Self::Post),
            "PUT" => Some(Self::Put),
            "PATCH" => Some(Self::Patch),
            "DELETE" => Some(Self::Delete),
            _ => None,
        }
    }
}

impl Default for HttpMethod {
    fn default() -> Self {
        Self::Post
    }
}

/// Authentication material attached to an attack request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthMaterial {
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub bearer_token: Option<String>,
    pub basic_username: Option<String>,
    pub basic_password: Option<String>,
    pub cookie_header: Option<String>,
    pub storage_state_path: Option<String>,
    pub api_key_header: Option<String>,
    pub api_key: Option<String>,
    pub query_key_name: Option<String>,
    pub query_key_value: Option<String>,
    pub aws_access_key_id: Option<String>,
    pub aws_secret_access_key: Option<String>,
    pub aws_session_token: Option<String>,
    pub aws_region: Option<String>,
    pub aws_service: Option<String>,
}

impl AuthMaterial {
    pub fn apply_to_headers(&self, headers: &mut HashMap<String, String>) {
        for (key, value) in &self.headers {
            headers.insert(key.clone(), value.to_string());
        }
        if let Some(token) = &self.bearer_token {
            if !headers.keys().any(|k| k.eq_ignore_ascii_case("authorization")) {
                headers.insert("Authorization".into(), format!("Bearer {token}"));
            }
        }
        if let (Some(user), Some(pass)) = (&self.basic_username, &self.basic_password) {
            let encoded = base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                format!("{user}:{pass}"),
            );
            headers.insert("Authorization".into(), format!("Basic {encoded}"));
        }
        if let Some(cookie) = &self.cookie_header {
            headers.insert("Cookie".into(), cookie.clone());
        }
        if let (Some(header), Some(key)) = (&self.api_key_header, &self.api_key) {
            if !header.trim().is_empty() && !key.is_empty() {
                headers.insert(header.clone(), key.clone());
            }
        } else if let Some(key) = &self.api_key {
            if !headers.keys().any(|k| k.eq_ignore_ascii_case("x-api-key")) {
                headers.insert("x-api-key".into(), key.clone());
            }
        }
    }

    pub fn apply_query_key(&self, url: &str) -> String {
        let (Some(name), Some(value)) = (&self.query_key_name, &self.query_key_value) else {
            return url.to_string();
        };
        if name.trim().is_empty() || value.is_empty() {
            return url.to_string();
        }
        let Ok(mut parsed) = url::Url::parse(url) else {
            return url.to_string();
        };
        parsed.query_pairs_mut().append_pair(name, value);
        parsed.to_string()
    }
}

/// Target I/O request. Attack is one caller; product inference shares this type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttackRequest {
    #[serde(default)]
    pub purpose: HarnessPurpose,
    pub url: String,
    #[serde(default)]
    pub method: HttpMethod,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub payload: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub auth: AuthMaterial,
    /// Optional chat UI selectors for Playwright harness.
    #[serde(default)]
    pub chat_selectors: HashMap<String, String>,
    #[serde(default)]
    pub stream: bool,
    pub conversation_id: Option<String>,
    pub mcp_method: Option<String>,
    pub mcp_session_id: Option<String>,
    pub ws_subprotocol: Option<String>,
    pub file_path: Option<String>,
    #[serde(default)]
    pub keep_page: bool,
    #[serde(default)]
    pub wait_stable_ms: u64,
    #[serde(default)]
    pub messages: Vec<ChatMessage>,
    pub system: Option<String>,
    #[serde(default)]
    pub tools: Vec<ChatTool>,
    pub tool_choice: Option<serde_json::Value>,
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    #[serde(skip)]
    pub cancel: CancelFlag,
    #[serde(skip)]
    pub stream_tx: Option<Arc<UnboundedSender<StreamChunk>>>,
}

fn default_timeout_ms() -> u64 {
    30_000
}

impl Default for AttackRequest {
    fn default() -> Self {
        Self {
            purpose: HarnessPurpose::attack(),
            url: String::new(),
            method: HttpMethod::Post,
            headers: HashMap::new(),
            body: None,
            payload: String::new(),
            timeout_ms: default_timeout_ms(),
            auth: AuthMaterial::default(),
            chat_selectors: HashMap::new(),
            stream: false,
            conversation_id: None,
            mcp_method: None,
            mcp_session_id: None,
            ws_subprotocol: None,
            file_path: None,
            keep_page: false,
            wait_stable_ms: 0,
            messages: Vec::new(),
            system: None,
            tools: Vec::new(),
            tool_choice: None,
            model: None,
            max_tokens: None,
            temperature: None,
            cancel: CancelFlag::new(),
            stream_tx: None,
        }
    }
}

impl AttackRequest {
    pub fn from_payload(url: impl Into<String>, payload: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            payload: payload.into(),
            ..Self::default()
        }
    }

    pub fn from_chat(url: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self {
            purpose: HarnessPurpose::assistant(),
            url: url.into(),
            messages,
            timeout_ms: 120_000,
            ..Self::default()
        }
    }

    pub fn with_auth(mut self, auth: AuthMaterial) -> Self {
        self.auth = auth;
        self
    }

    pub fn with_purpose(mut self, purpose: HarnessPurpose) -> Self {
        self.purpose = purpose;
        self
    }

    pub fn has_chat_native(&self) -> bool {
        !self.messages.is_empty()
            || self.system.is_some()
            || !self.tools.is_empty()
            || self.model.is_some()
            || self.purpose.is_product_inference()
    }

    pub fn resolved_messages(&self) -> Vec<ChatMessage> {
        if !self.messages.is_empty() {
            return self.messages.clone();
        }
        let mut messages = Vec::new();
        if let Some(system) = self
            .system
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            messages.push(ChatMessage::text("system", system));
        }
        let user = if self.payload.is_empty() {
            self.body.clone().unwrap_or_default()
        } else {
            self.payload.clone()
        };
        messages.push(ChatMessage::text("user", user));
        messages
    }

    pub fn system_and_user_prompt(&self) -> (Option<String>, String) {
        let messages = self.resolved_messages();
        let system = messages
            .iter()
            .find(|message| message.role == "system")
            .map(|message| message.text_content())
            .filter(|text| !text.is_empty());
        let user = messages
            .iter()
            .filter(|message| message.role == "user")
            .map(ChatMessage::text_content)
            .collect::<Vec<_>>()
            .join("\n");
        (system, user)
    }

    pub fn openai_chat_body(&self) -> String {
        let messages: Vec<serde_json::Value> = self
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
            .collect();
        let mut body = serde_json::json!({
            "model": self.model.as_deref().unwrap_or("promptlab-probe"),
            "messages": messages,
        });
        if let Some(max_tokens) = self.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }
        if let Some(temperature) = self.temperature {
            body["temperature"] = serde_json::json!(temperature);
        }
        if self.stream {
            body["stream"] = serde_json::json!(true);
        }
        if !self.tools.is_empty() {
            body["tools"] = serde_json::json!(self
                .tools
                .iter()
                .map(|tool| serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.parameters,
                    }
                }))
                .collect::<Vec<_>>());
            body["tool_choice"] = self
                .tool_choice
                .clone()
                .unwrap_or_else(|| serde_json::json!("auto"));
        }
        if self.url.to_ascii_lowercase().contains("openrouter.ai") {
            body["include_reasoning"] = serde_json::json!(true);
        }
        body.to_string()
    }

    pub fn anthropic_chat_body(&self) -> String {
        let (system, messages) = split_system_messages(&self.resolved_messages());
        let mut body = serde_json::json!({
            "model": self.model.as_deref().unwrap_or("claude-3-5-sonnet-20241022"),
            "max_tokens": self.max_tokens.unwrap_or(256),
            "messages": messages
                .into_iter()
                .map(|message| serde_json::json!({
                    "role": message.role,
                    "content": message.content,
                }))
                .collect::<Vec<_>>(),
        });
        if let Some(system) = system {
            body["system"] = serde_json::Value::String(system);
        }
        if let Some(temperature) = self.temperature {
            body["temperature"] = serde_json::json!(temperature);
        }
        if self.stream {
            body["stream"] = serde_json::json!(true);
        }
        if !self.tools.is_empty() {
            body["tools"] = serde_json::json!(self
                .tools
                .iter()
                .map(|tool| serde_json::json!({
                    "name": tool.name,
                    "description": tool.description,
                    "input_schema": tool.parameters,
                }))
                .collect::<Vec<_>>());
        }
        body.to_string()
    }

    pub fn gemini_chat_body(&self) -> String {
        let (system, messages) = split_system_messages(&self.resolved_messages());
        let contents: Vec<serde_json::Value> = messages
            .into_iter()
            .map(|message| {
                let role = match message.role.as_str() {
                    "assistant" => "model",
                    _ => "user",
                };
                serde_json::json!({
                    "role": role,
                    "parts": [{ "text": message.text_content() }]
                })
            })
            .collect();
        let mut body = serde_json::json!({ "contents": contents });
        if let Some(system) = system {
            body["systemInstruction"] = serde_json::json!({
                "parts": [{ "text": system }]
            });
        }
        let mut generation = serde_json::Map::new();
        if let Some(temperature) = self.temperature {
            generation.insert("temperature".into(), serde_json::json!(temperature));
        }
        if let Some(max_tokens) = self.max_tokens {
            generation.insert("maxOutputTokens".into(), serde_json::json!(max_tokens));
        }
        if !generation.is_empty() {
            body["generationConfig"] = serde_json::Value::Object(generation);
        }
        if !self.tools.is_empty() {
            body["tools"] = serde_json::json!([{
                "functionDeclarations": self.tools.iter().map(|tool| serde_json::json!({
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters,
                })).collect::<Vec<_>>()
            }]);
        }
        body.to_string()
    }

    pub fn bedrock_converse_body(&self) -> String {
        let (system, messages) = split_system_messages(&self.resolved_messages());
        let mut body = serde_json::json!({
            "messages": messages
                .into_iter()
                .map(|message| serde_json::json!({
                    "role": if message.role == "assistant" { "assistant" } else { "user" },
                    "content": [{ "text": message.text_content() }]
                }))
                .collect::<Vec<_>>(),
        });
        if let Some(system) = system {
            body["system"] = serde_json::json!([{ "text": system }]);
        }
        let mut inference = serde_json::Map::new();
        inference.insert(
            "maxTokens".into(),
            serde_json::json!(self.max_tokens.unwrap_or(256)),
        );
        if let Some(temperature) = self.temperature {
            inference.insert("temperature".into(), serde_json::json!(temperature));
        }
        body["inferenceConfig"] = serde_json::Value::Object(inference);
        body.to_string()
    }

    pub fn effective_body(&self) -> String {
        if let Some(body) = &self.body {
            let mut rendered = body.replace("{{payload}}", &self.payload);
            rendered = rendered.replace("{{PAYLOAD}}", &self.payload);
            if let Some(conversation_id) = &self.conversation_id {
                rendered = rendered.replace("{{conversation_id}}", conversation_id);
            }
            rendered
        } else {
            self.payload.clone()
        }
    }

    pub fn merged_headers(&self) -> HashMap<String, String> {
        let mut headers = self.headers.clone();
        self.auth.apply_to_headers(&mut headers);
        if !matches!(self.method, HttpMethod::Get)
            && !headers.keys().any(|k| k.eq_ignore_ascii_case("content-type"))
        {
            headers.insert("Content-Type".into(), "application/json".into());
        }
        headers
    }

    pub fn emit_stream(&self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }
        self.emit_chunk(StreamChunk::text(chunk));
    }

    pub fn emit_chunk(&self, chunk: StreamChunk) {
        if chunk.is_empty_text() {
            return;
        }
        if let Some(tx) = &self.stream_tx {
            let _ = tx.send(chunk);
        }
    }

    pub fn emit_finish(&self, stop_reason: Option<String>, error_class: Option<String>) {
        self.emit_chunk(StreamChunk::finish(stop_reason, error_class));
    }
}

fn split_system_messages(messages: &[ChatMessage]) -> (Option<String>, Vec<ChatMessage>) {
    let system = messages
        .iter()
        .filter(|message| message.role == "system")
        .map(ChatMessage::text_content)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let rest = messages
        .iter()
        .filter(|message| message.role != "system")
        .cloned()
        .collect();
    (if system.is_empty() { None } else { Some(system) }, rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_id_maps_to_purpose() {
        assert_eq!(HarnessPurpose::from_agent_id("yazg"), HarnessPurpose::assistant());
        assert_eq!(HarnessPurpose::from_agent_id("attack_plan"), HarnessPurpose::planner());
        assert_eq!(HarnessPurpose::from_agent_id("generate_prompt"), HarnessPurpose::generator());
        assert_eq!(HarnessPurpose::from_agent_id("summary"), HarnessPurpose::report());
        assert_eq!(
            HarnessPurpose::from_agent_id("agentic_attack_execution"),
            HarnessPurpose::planner()
        );
        assert_eq!(
            HarnessPurpose::from_agent_id("judge_coordinator"),
            HarnessPurpose::planner()
        );
        assert!(HarnessPurpose::assistant().is_product_inference());
        assert!(HarnessPurpose::planner().fails_on_retryable_http());
        assert!(!HarnessPurpose::attack().fails_on_retryable_http());
        assert!(!HarnessPurpose::attack().is_product_inference());
    }

    #[test]
    fn openai_chat_body_uses_messages_and_tools() {
        let mut request = AttackRequest::from_chat(
            "https://openrouter.ai/api/v1/chat/completions",
            vec![ChatMessage::text("user", "hi")],
        );
        request.model = Some("openai/gpt-4o".into());
        request.max_tokens = Some(32);
        request.tools.push(ChatTool {
            name: "lookup".into(),
            description: "look up".into(),
            parameters: serde_json::json!({"type":"object"}),
        });
        let body: serde_json::Value = serde_json::from_str(&request.openai_chat_body()).unwrap();
        assert_eq!(body["model"], "openai/gpt-4o");
        assert_eq!(body["include_reasoning"], true);
        assert_eq!(body["tools"][0]["function"]["name"], "lookup");
    }
}
