use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Response shape consumed exclusively by the Judge Engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
        let content = extract_display_content(&body);
        Self {
            content: content.clone(),
            raw_response: body,
            status_code: Some(status),
            headers,
            metadata: HashMap::from([
                ("harness".into(), harness.into()),
                ("transport".into(), "http".into()),
            ]),
        }
    }

    pub fn from_chat(response_text: String, harness: &str) -> Self {
        Self {
            content: response_text.clone(),
            raw_response: response_text,
            status_code: None,
            headers: HashMap::new(),
            metadata: HashMap::from([
                ("harness".into(), harness.into()),
                ("transport".into(), "playwright_chat".into()),
            ]),
        }
    }

    /// Text passed to the judge LLM — prefers extracted assistant content, falls back to raw body.
    pub fn judge_text(&self) -> String {
        let trimmed = self.content.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
        extract_display_content(self.raw_response.trim())
    }
}

fn extract_display_content(body: &str) -> String {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(content) = json
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
        {
            return content.to_string();
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
    fn judge_text_falls_back_to_raw_body() {
        let normalized = NormalizedResponse {
            content: String::new(),
            raw_response: "plain text response".into(),
            status_code: Some(200),
            headers: Default::default(),
            metadata: Default::default(),
        };
        assert_eq!(normalized.judge_text(), "plain text response");
    }
}
