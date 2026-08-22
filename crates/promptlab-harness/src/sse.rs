use serde_json::Value;

/// Accumulate Server-Sent Event payloads into assistant text.
pub fn consume_sse_chunk(buffer: &mut String, chunk: &str, assembled: &mut String) {
    buffer.push_str(chunk);
    while let Some(idx) = buffer.find("\n\n") {
        let frame = buffer[..idx].to_string();
        buffer.drain(..=idx + 1);
        for line in frame.lines() {
            let line = line.trim();
            let payload = line.strip_prefix("data:").map(str::trim).unwrap_or("");
            if payload.is_empty() || payload == "[DONE]" {
                continue;
            }
            if let Ok(json) = serde_json::from_str::<Value>(payload) {
                if let Some(text) = delta_text(&json) {
                    assembled.push_str(&text);
                }
            } else if !payload.starts_with('{') {
                assembled.push_str(payload);
            }
        }
    }
}

pub fn delta_text(json: &Value) -> Option<String> {
    if let Some(text) = json
        .pointer("/choices/0/delta/content")
        .and_then(Value::as_str)
    {
        return Some(text.to_string());
    }
    if let Some(text) = json.pointer("/delta/text").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(text) = json
        .pointer("/content_block/text")
        .and_then(Value::as_str)
    {
        return Some(text.to_string());
    }
    if json.get("event").and_then(Value::as_str) == Some("message")
        || json.get("event").and_then(Value::as_str) == Some("agent_message")
    {
        if let Some(text) = json.get("answer").and_then(Value::as_str) {
            return Some(text.to_string());
        }
    }
    if let Some(text) = json.get("answer").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(text) = json
        .pointer("/candidates/0/content/parts/0/text")
        .and_then(Value::as_str)
    {
        return Some(text.to_string());
    }
    None
}

pub fn request_wants_stream(body: Option<&str>, stream_flag: bool) -> bool {
    if stream_flag {
        return true;
    }
    let Some(body) = body else {
        return false;
    };
    if body.contains("\"stream\": true") || body.contains("\"stream\":true") {
        return true;
    }
    body.contains("\"response_mode\": \"streaming\"")
        || body.contains("\"response_mode\":\"streaming\"")
}
