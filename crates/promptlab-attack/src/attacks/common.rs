use regex::Regex;
use serde_json::Value;

/// Extract assistant-visible text from common LLM JSON response shapes.
pub fn extract_response_text(body: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        if let Some(text) = value
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
        {
            return text.to_string();
        }
        if let Some(text) = value.pointer("/content/0/text").and_then(|v| v.as_str()) {
            return text.to_string();
        }
        if let Some(text) = value.pointer("/candidates/0/content/parts/0/text").and_then(|v| v.as_str()) {
            return text.to_string();
        }
        if let Some(text) = value.get("output").and_then(|v| v.as_str()) {
            return text.to_string();
        }
        if let Some(text) = value.get("response").and_then(|v| v.as_str()) {
            return text.to_string();
        }
    }
    body.to_string()
}

pub fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    let lower = haystack.to_lowercase();
    needles.iter().any(|n| lower.contains(&n.to_lowercase()))
}

pub fn matching_indicators(text: &str, patterns: &[(&str, &str)]) -> Vec<String> {
    patterns
        .iter()
        .filter_map(|(label, pattern)| {
            Regex::new(&format!("(?i){pattern}"))
                .ok()
                .filter(|re| re.is_match(text))
                .map(|_| (*label).to_string())
        })
        .collect()
}

pub fn json_pointer_exists(body: &str, pointer: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return false;
    };
    value
        .pointer(pointer)
        .is_some_and(|v| !v.is_null())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_openai_content() {
        let body = r#"{"choices":[{"message":{"content":"hello world"}}]}"#;
        assert_eq!(extract_response_text(body), "hello world");
    }
}
