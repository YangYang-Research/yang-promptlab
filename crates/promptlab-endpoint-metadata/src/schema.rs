use regex::Regex;
use std::sync::LazyLock;

use crate::types::{
    InferenceFields, NormalizedSchema, SchemaField, SchemaMetadata, TransportKind,
};

pub struct SchemaInferenceEngine;

static OPENAI_CHAT_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)/v1/(chat/)?completions?").unwrap());
static EMBEDDINGS_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)/v1/embeddings?").unwrap());
static IMAGES_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)/v1/images?").unwrap());
static AUDIO_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)/v1/audio").unwrap());
static MODERATIONS_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)/v1/moderations?").unwrap());
static GRAPHQL_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)/graphql").unwrap());
static MCP_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)/mcp").unwrap());

impl SchemaInferenceEngine {
    pub fn infer(
        url: &str,
        method: &str,
        content_type: Option<&str>,
        request_body: Option<&str>,
        response_body: Option<&str>,
        api_style: &str,
    ) -> (SchemaMetadata, InferenceFields) {
        let mut schema = SchemaMetadata::default();
        let inference;

        let ct = content_type.unwrap_or_default().to_ascii_lowercase();
        if ct.contains("multipart") {
            schema.transport.push(TransportKind::Multipart);
        } else if ct.contains("json") || request_body.map(|b| b.trim().starts_with('{')).unwrap_or(false)
        {
            schema.transport.push(TransportKind::Json);
            schema.content_type = Some("application/json".into());
        }

        if GRAPHQL_PATH.is_match(url) || request_body.is_some_and(|b| b.contains("\"query\"")) {
            schema.transport.push(TransportKind::Graphql);
        }
        if MCP_PATH.is_match(url) {
            schema.transport.push(TransportKind::Mcp);
        }
        if url.starts_with("ws://") || url.starts_with("wss://") {
            schema.transport.push(TransportKind::Websocket);
        }
        if ct.contains("text/event-stream") {
            schema.transport.push(TransportKind::Sse);
        }

        let request_fields = infer_request_fields(url, method, api_style, request_body);
        schema.request_schema = Some(NormalizedSchema {
            format: schema_format(&schema.transport),
            fields: request_fields.clone(),
        });
        inference = fields_to_inference(&request_fields);

        if let Some(body) = response_body {
            schema.response_schema = Some(infer_response_schema(body));
        }

        (schema, inference)
    }
}

fn schema_format(transport: &[TransportKind]) -> String {
    if transport.contains(&TransportKind::Graphql) {
        "graphql".into()
    } else if transport.contains(&TransportKind::Multipart) {
        "multipart".into()
    } else {
        "json".into()
    }
}

fn infer_request_fields(
    url: &str,
    method: &str,
    api_style: &str,
    body: Option<&str>,
) -> Vec<SchemaField> {
    let mut fields = Vec::new();

    if let Some(raw) = body {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
            if let Some(obj) = value.as_object() {
                for (name, val) in obj {
                    fields.push(SchemaField {
                        name: name.clone(),
                        field_type: json_type_name(val),
                        required: true,
                    });
                }
                return fields;
            }
        }
    }

    // Path + API-style heuristics when no body observed.
    if EMBEDDINGS_PATH.is_match(url) {
        fields.push(field("input", "string", true));
        fields.push(field("model", "string", false));
        return fields;
    }
    if IMAGES_PATH.is_match(url) {
        fields.push(field("prompt", "string", true));
        fields.push(field("model", "string", false));
        return fields;
    }
    if AUDIO_PATH.is_match(url) {
        fields.push(field("file", "string", true));
        fields.push(field("model", "string", false));
        return fields;
    }
    if MODERATIONS_PATH.is_match(url) {
        fields.push(field("input", "string", true));
        fields.push(field("model", "string", false));
        return fields;
    }

    if OPENAI_CHAT_PATH.is_match(url)
        || api_style == "openai_compatible"
        || api_style == "anthropic_messages"
    {
        if api_style == "anthropic_messages" {
            fields.push(field("messages", "array", true));
            fields.push(field("model", "string", true));
            fields.push(field("max_tokens", "number", false));
            fields.push(field("stream", "boolean", false));
            fields.push(field("tools", "array", false));
        } else {
            fields.push(field("messages", "array", true));
            fields.push(field("model", "string", false));
            fields.push(field("stream", "boolean", false));
            fields.push(field("tools", "array", false));
            fields.push(field("response_format", "object", false));
        }
        return fields;
    }

    if method.eq_ignore_ascii_case("POST") {
        fields.push(field("prompt", "string", false));
        fields.push(field("messages", "array", false));
    }

    fields
}

fn infer_response_schema(body: &str) -> NormalizedSchema {
    let mut fields = Vec::new();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(obj) = value.as_object() {
            for (name, val) in obj {
                fields.push(SchemaField {
                    name: name.clone(),
                    field_type: json_type_name(val),
                    required: false,
                });
            }
        }
    }
    NormalizedSchema {
        format: "json".into(),
        fields,
    }
}

fn fields_to_inference(fields: &[SchemaField]) -> InferenceFields {
    let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
    InferenceFields {
        prompt_field: find_field(&names, &["prompt", "input", "query", "text"]),
        history_field: find_field(&names, &["messages", "history", "chat_history"]),
        conversation_field: find_field(&names, &["conversation_id", "session_id", "thread_id"]),
        model_field: find_field(&names, &["model", "model_id"]),
        stream_field: find_field(&names, &["stream", "streaming"]),
        tool_field: find_field(&names, &["tools", "functions", "tool_choice"]),
        attachment_field: find_field(&names, &["file", "image", "images", "attachments"]),
    }
}

fn find_field(names: &[&str], candidates: &[&str]) -> Option<String> {
    for candidate in candidates {
        if names.iter().any(|n| n.eq_ignore_ascii_case(candidate)) {
            return Some((*candidate).into());
        }
    }
    None
}

fn field(name: &str, field_type: &str, required: bool) -> SchemaField {
    SchemaField {
        name: name.into(),
        field_type: field_type.into(),
        required,
    }
}

fn json_type_name(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".into(),
        serde_json::Value::Bool(_) => "boolean".into(),
        serde_json::Value::Number(_) => "number".into(),
        serde_json::Value::String(_) => "string".into(),
        serde_json::Value::Array(_) => "array".into(),
        serde_json::Value::Object(_) => "object".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_openai_chat_fields() {
        let (schema, inference) = SchemaInferenceEngine::infer(
            "https://api.example.com/v1/chat/completions",
            "POST",
            Some("application/json"),
            None,
            None,
            "openai_compatible",
        );
        assert!(schema.transport.contains(&TransportKind::Json));
        assert_eq!(inference.history_field.as_deref(), Some("messages"));
        assert_eq!(inference.model_field.as_deref(), Some("model"));
    }
}
