use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Tool/function call extracted from a target response for judge/tool-abuse.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedToolCall {
    pub id: Option<String>,
    pub name: String,
    pub arguments: String,
}

/// Response shape consumed exclusively by the Judge Engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedResponse {
    pub content: String,
    pub raw_response: String,
    pub status_code: Option<u16>,
    /// Wire HTTP response headers from the target (not harness metadata).
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    #[serde(default)]
    pub tool_calls: Vec<NormalizedToolCall>,
    pub stop_reason: Option<String>,
    pub conversation_id: Option<String>,
    /// `http` | `auth` | `transport` | `model_refusal` | `cancelled`
    pub error_class: Option<String>,
    pub usage_input_tokens: Option<u64>,
    pub usage_output_tokens: Option<u64>,
}

impl NormalizedResponse {
    pub fn from_http(status: u16, body: String, harness: &str) -> Self {
        Self::from_http_headers(status, HashMap::new(), body, harness)
    }

    pub fn from_http_headers(
        status: u16,
        headers: HashMap<String, String>,
        body: String,
        harness: &str,
    ) -> Self {
        let mut response = Self {
            content: extract_display_content(&body),
            raw_response: body.clone(),
            status_code: Some(status),
            headers,
            metadata: HashMap::from([
                ("harness".into(), harness.into()),
                ("transport".into(), "http".into()),
            ]),
            error_class: classify_http(status),
            ..Self::default()
        };
        enrich_from_json(&mut response, &body);
        if response.error_class.is_none() {
            response.error_class = classify_empty(&response);
        }
        response
    }

    pub fn from_chat(response_text: String, harness: &str) -> Self {
        Self {
            content: response_text.clone(),
            raw_response: response_text,
            metadata: HashMap::from([
                ("harness".into(), harness.into()),
                ("transport".into(), "playwright_chat".into()),
            ]),
            ..Self::default()
        }
    }

    pub fn transport_error(message: String, harness: &str) -> Self {
        Self {
            content: String::new(),
            raw_response: message.clone(),
            metadata: HashMap::from([
                ("harness".into(), harness.into()),
                ("error".into(), "transport".into()),
            ]),
            error_class: Some("transport".into()),
            ..Self::default()
        }
    }

    /// Text passed to the judge LLM — prefers extracted assistant content, falls back to raw body.
    pub fn judge_text(&self) -> String {
        let trimmed = self.content.trim();
        if !trimmed.is_empty() {
            let mut text = trimmed.to_string();
            if !self.tool_calls.is_empty() {
                text.push_str("\n\n[tool_calls]\n");
                for call in &self.tool_calls {
                    text.push_str(&format!("{}: {}\n", call.name, call.arguments));
                }
            }
            return text;
        }
        extract_display_content(self.raw_response.trim())
    }
}

/// Extract a provider error message from RFC 7807 / OpenAI-style error JSON.
/// Returns `None` when the body is not a provider error payload.
pub fn provider_error_detail(body: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(body.trim()).ok()?;
    if json.get("isAiEndpoint").is_some() || json.get("is_ai_endpoint").is_some() {
        return None;
    }
    if let Some(message) = json
        .pointer("/error/message")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        return Some(message.to_string());
    }
    let status = json.get("status").and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
    });
    let detail = json
        .get("detail")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty());
    let title = json
        .get("title")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty());
    if let Some(status) = status.filter(|code| *code >= 400) {
        if let Some(detail) = detail {
            return Some(detail.to_string());
        }
        if let Some(title) = title {
            return Some(format!("HTTP {status} {title}"));
        }
        return Some(format!("HTTP {status}"));
    }
    if title == Some("Gone") {
        return Some(detail.unwrap_or("Gone").to_string());
    }
    None
}

pub fn classify_http(status: u16) -> Option<String> {
    match status {
        0 => Some("transport".into()),
        200..=299 => None,
        401 | 403 => Some("auth".into()),
        408 | 504 => Some("timeout".into()),
        429 => Some("rate_limit".into()),
        _ => Some("http".into()),
    }
}

pub fn classify_empty(response: &NormalizedResponse) -> Option<String> {
    if response.error_class.is_some() {
        return response.error_class.clone();
    }
    let ok = response
        .status_code
        .map(|status| (200..300).contains(&status))
        .unwrap_or(true);
    if ok && response.content.trim().is_empty() && response.tool_calls.is_empty() {
        Some("empty".into())
    } else {
        None
    }
}

fn enrich_from_json(response: &mut NormalizedResponse, body: &str) {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(body) else {
        return;
    };
    response.tool_calls = extract_tool_calls(&json);
    response.stop_reason = json
        .pointer("/choices/0/finish_reason")
        .and_then(|v| v.as_str())
        .or_else(|| json.get("stop_reason").and_then(|v| v.as_str()))
        .or_else(|| json.get("finishReason").and_then(|v| v.as_str()))
        .map(str::to_string);
    response.conversation_id = json
        .get("conversation_id")
        .or_else(|| json.get("conversationId"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    response.usage_input_tokens = json
        .pointer("/usage/prompt_tokens")
        .or_else(|| json.pointer("/usage/input_tokens"))
        .or_else(|| json.pointer("/usage/inputTokens"))
        .or_else(|| json.pointer("/usageMetadata/promptTokenCount"))
        .and_then(|v| v.as_u64());
    response.usage_output_tokens = json
        .pointer("/usage/completion_tokens")
        .or_else(|| json.pointer("/usage/output_tokens"))
        .or_else(|| json.pointer("/usage/outputTokens"))
        .or_else(|| json.pointer("/usageMetadata/candidatesTokenCount"))
        .and_then(|v| v.as_u64());
}

fn extract_tool_calls(json: &serde_json::Value) -> Vec<NormalizedToolCall> {
    let mut calls = Vec::new();
    if let Some(items) = json.pointer("/choices/0/message/tool_calls").and_then(|v| v.as_array()) {
        for item in items {
            let name = item
                .pointer("/function/name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if name.is_empty() {
                continue;
            }
            let arguments = item
                .pointer("/function/arguments")
                .and_then(|v| v.as_str())
                .unwrap_or("{}")
                .to_string();
            calls.push(NormalizedToolCall {
                id: item.get("id").and_then(|v| v.as_str()).map(str::to_string),
                name,
                arguments,
            });
        }
    }
    if let Some(blocks) = json.get("content").and_then(|v| v.as_array()) {
        for block in blocks {
            if block.get("type").and_then(|v| v.as_str()) != Some("tool_use") {
                continue;
            }
            let name = block
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if name.is_empty() {
                continue;
            }
            let arguments = block
                .get("input")
                .map(|v| v.to_string())
                .unwrap_or_else(|| "{}".into());
            calls.push(NormalizedToolCall {
                id: block.get("id").and_then(|v| v.as_str()).map(str::to_string),
                name,
                arguments,
            });
        }
    }
    if let Some(parts) = json
        .pointer("/candidates/0/content/parts")
        .and_then(|v| v.as_array())
    {
        for part in parts {
            if let Some(call) = part.get("functionCall") {
                let name = call
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if name.is_empty() {
                    continue;
                }
                let arguments = call
                    .get("args")
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "{}".into());
                calls.push(NormalizedToolCall {
                    id: None,
                    name,
                    arguments,
                });
            }
        }
    }
    calls
}

fn extract_display_content(body: &str) -> String {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        let message = json.pointer("/choices/0/message");
        if let Some(content) = message
            .and_then(|msg| msg.get("content"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|text| !text.is_empty())
        {
            return content.to_string();
        }
        if let Some(text) = message.and_then(|msg| text_from_content_array(msg.get("content"))) {
            return text;
        }
        for key in ["reasoning", "reasoning_content"] {
            if let Some(text) = message
                .and_then(|msg| msg.get(key))
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|text| !text.is_empty())
            {
                return text.to_string();
            }
        }
        if let Some(details) = message
            .and_then(|msg| msg.get("reasoning_details"))
            .and_then(|v| v.as_array())
        {
            let parts: Vec<&str> = details
                .iter()
                .filter_map(|item| item.get("text").and_then(|v| v.as_str()))
                .collect();
            if !parts.is_empty() {
                return parts.join("\n");
            }
        }
        if let Some(text) = text_from_content_array(json.pointer("/choices/0/message/content")) {
            return text;
        }
        if let Some(text) = text_from_content_array(json.get("content")) {
            return text;
        }
        if let Some(content) = json.pointer("/content/0/text").and_then(|v| v.as_str()) {
            return content.to_string();
        }
        if let Some(text) = json
            .pointer("/output/message/content/0/text")
            .and_then(|v| v.as_str())
        {
            return text.to_string();
        }
        if let Some(text) = json
            .pointer("/candidates/0/content/parts/0/text")
            .and_then(|v| v.as_str())
        {
            return text.to_string();
        }
        if let Some(text) = json.pointer("/data/answer").and_then(|v| v.as_str()) {
            return text.to_string();
        }
        if let Some(text) = json.get("answer").and_then(|v| v.as_str()) {
            return text.to_string();
        }
        if let Some(text) = json.get("response").and_then(|v| v.as_str()) {
            return text.to_string();
        }
        if let Some(text) = json.get("output").and_then(|v| v.as_str()) {
            return text.to_string();
        }
        if let Some(text) = json
            .pointer("/result/content/0/text")
            .and_then(|v| v.as_str())
        {
            return text.to_string();
        }
    }
    body.to_string()
}

fn text_from_content_array(value: Option<&serde_json::Value>) -> Option<String> {
    let blocks = value?.as_array()?;
    let parts: Vec<&str> = blocks
        .iter()
        .filter_map(|block| {
            block
                .get("text")
                .and_then(|v| v.as_str())
                .or_else(|| block.as_str())
        })
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_openai_content() {
        let body = r#"{"choices":[{"message":{"content":"hello"}}]}"#;
        let normalized = NormalizedResponse::from_http(200, body.into(), "openai");
        assert_eq!(normalized.content, "hello");
        assert_eq!(normalized.judge_text(), "hello");
    }

    #[test]
    fn extracts_openai_multimodal_content_array() {
        let body = r#"{"choices":[{"message":{"content":[{"type":"text","text":"hello array"}]}}]}"#;
        let normalized = NormalizedResponse::from_http(200, body.into(), "openai");
        assert_eq!(normalized.judge_text(), "hello array");
    }

    #[test]
    fn extracts_anthropic_tool_use() {
        let body = r#"{"content":[{"type":"tool_use","id":"t1","name":"bash","input":{"cmd":"id"}}],"stop_reason":"tool_use"}"#;
        let normalized = NormalizedResponse::from_http(200, body.into(), "anthropic");
        assert_eq!(normalized.tool_calls.len(), 1);
        assert_eq!(normalized.tool_calls[0].name, "bash");
        assert_eq!(normalized.stop_reason.as_deref(), Some("tool_use"));
        assert!(normalized.judge_text().contains("bash"));
    }

    #[test]
    fn judge_text_falls_back_to_raw_body() {
        let normalized = NormalizedResponse {
            content: String::new(),
            raw_response: "plain text response".into(),
            ..Default::default()
        };
        assert_eq!(normalized.judge_text(), "plain text response");
    }

    #[test]
    fn provider_error_detail_reads_rfc7807_gone() {
        let body = r#"{"type":"about:blank","title":"Gone","status":410,"detail":"The model 'meta/llama-3.1-8b-instruct' has reached its end of life on 2026-08-26T09:00:00Z and is no longer available."}"#;
        assert!(
            provider_error_detail(body)
                .expect("detail")
                .contains("end of life")
        );
    }

    #[test]
    fn provider_error_detail_reads_openai_error() {
        let body = r#"{"error":{"message":"model_not_found","type":"invalid_request_error"}}"#;
        assert_eq!(
            provider_error_detail(body).as_deref(),
            Some("model_not_found")
        );
    }

    #[test]
    fn provider_error_detail_ignores_classifier_json() {
        let body = r#"{"isAiEndpoint":true,"confidence":0.9,"rationale":"assistant reply"}"#;
        assert_eq!(provider_error_detail(body), None);
    }
}
