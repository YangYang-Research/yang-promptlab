use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Response shape consumed exclusively by the Judge Engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedResponse {
    pub content: String,
    pub raw_response: String,
    pub status_code: Option<u16>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl NormalizedResponse {
    pub fn from_http(status: u16, body: String, harness: &str) -> Self {
        let content = extract_display_content(&body);
        Self {
            content: content.clone(),
            raw_response: body,
            status_code: Some(status),
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
            metadata: HashMap::from([
                ("harness".into(), harness.into()),
                ("transport".into(), "playwright_chat".into()),
            ]),
        }
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
        if let Some(content) = json.pointer("/content/0/text").and_then(|v| v.as_str()) {
            return content.to_string();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_openai_content() {
        let body = r#"{"choices":[{"message":{"content":"hello"}}]}"#;
        let normalized = NormalizedResponse::from_http(200, body.into(), "openai");
        assert_eq!(normalized.content, "hello");
    }
}
