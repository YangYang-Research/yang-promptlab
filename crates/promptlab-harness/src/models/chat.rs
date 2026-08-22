use serde::{Deserialize, Serialize};

/// One chat message on the harness request (OpenAI-shaped, used by all product-inference callers).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default)]
    pub content: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
}

impl ChatMessage {
    pub fn text(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: serde_json::Value::String(content.into()),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }
    }

    pub fn from_json(value: &serde_json::Value) -> Self {
        Self {
            role: value
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("user")
                .to_string(),
            content: value
                .get("content")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
            name: value
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            tool_call_id: value
                .get("tool_call_id")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            tool_calls: value.get("tool_calls").cloned(),
        }
    }

    pub fn text_content(&self) -> String {
        match &self.content {
            serde_json::Value::String(text) => text.clone(),
            serde_json::Value::Array(parts) => parts
                .iter()
                .filter_map(|part| {
                    part.get("text")
                        .and_then(|v| v.as_str())
                        .or_else(|| part.as_str())
                })
                .collect::<Vec<_>>()
                .join("\n"),
            other if other.is_null() => String::new(),
            other => other.to_string(),
        }
    }
}

/// Tool schema bound on a chat-native completion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatTool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Stream protocol for `AttackRequest::stream_tx`. Retry frames are never emitted here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamChunk {
    Text {
        delta: String,
    },
    ToolCall {
        id: Option<String>,
        name: String,
        arguments_delta: String,
    },
    Usage {
        input_tokens: u64,
        output_tokens: u64,
    },
    Finish {
        stop_reason: Option<String>,
        error_class: Option<String>,
    },
}

impl StreamChunk {
    pub fn text(delta: impl Into<String>) -> Self {
        Self::Text {
            delta: delta.into(),
        }
    }

    pub fn finish(stop_reason: Option<String>, error_class: Option<String>) -> Self {
        Self::Finish {
            stop_reason,
            error_class,
        }
    }

    pub fn is_empty_text(&self) -> bool {
        matches!(self, Self::Text { delta } if delta.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_content_joins_array_parts() {
        let message = ChatMessage {
            role: "assistant".into(),
            content: serde_json::json!([{"type":"text","text":"hello"},{"type":"text","text":"world"}]),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        };
        assert_eq!(message.text_content(), "hello\nworld");
    }
}
