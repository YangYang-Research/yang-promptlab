use regex::Regex;
use std::sync::OnceLock;

use crate::types::{DiscoveredEndpoint, EndpointKind, HttpSnapshot};

fn ai_path_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(concat!(
            r"(?i)/(",
            r"v1/chat/completions|v1/completions|v1/embeddings|v1/models|",
            r"api/chat|api/generate|api/llm|api/ai|anthropic/v1/messages|",
            r"openai/deployments|/invoke|/predict|/inference",
            r")(?:/|$|\?)"
        ))
        .expect("regex")
    })
}

pub fn detect_ai_from_snapshot(
    snapshot: &HttpSnapshot,
    source_url: Option<&str>,
) -> Vec<DiscoveredEndpoint> {
    let mut found = Vec::new();

    if ai_path_re().is_match(&snapshot.url) && snapshot.status != 404 {
        let confidence = classify_ai_confidence(snapshot);
        let mut ep = DiscoveredEndpoint::new(
            &snapshot.url,
            EndpointKind::AiEndpoint,
            confidence,
            "URL matches known AI/LLM endpoint pattern",
        )
        .with_method(infer_ai_method(snapshot));
        if let Some(src) = source_url {
            ep = ep.with_source(src);
        }
        found.push(ep);
    }

    if snapshot.is_success() && snapshot.is_json() && looks_like_models_list(&snapshot.body) {
        let mut ep = DiscoveredEndpoint::new(
            &snapshot.url,
            EndpointKind::AiEndpoint,
            0.85,
            "JSON body resembles OpenAI-compatible /v1/models listing",
        )
        .with_method("GET");
        if let Some(src) = source_url {
            ep = ep.with_source(src);
        }
        found.push(ep);
    }

    if snapshot.is_success() && openai_error_shape(&snapshot.body) {
        let mut ep = DiscoveredEndpoint::new(
            &snapshot.url,
            EndpointKind::AiEndpoint,
            0.9,
            "OpenAI-compatible error response shape",
        )
        .with_method("POST");
        if let Some(src) = source_url {
            ep = ep.with_source(src);
        }
        found.push(ep);
    }

    found
}

pub async fn probe_ai_paths(
    client: &crate::client::HttpClient,
    origin: &str,
) -> Vec<DiscoveredEndpoint> {
    use crate::detectors::paths::ai_probe_paths;

    let mut found = Vec::new();
    for url in ai_probe_paths(origin) {
        if let Ok(snapshot) = client.get(&url).await {
            found.extend(detect_ai_from_snapshot(&snapshot, Some(origin)));
            continue;
        }
        // Many AI endpoints only accept POST
        if let Ok(snapshot) = client
            .post_json(&url, r#"{"model":"probe","messages":[]}"#)
            .await
        {
            found.extend(detect_ai_from_snapshot(&snapshot, Some(origin)));
        }
    }
    found
}

fn classify_ai_confidence(snapshot: &HttpSnapshot) -> f32 {
    if snapshot.is_success() {
        0.9
    } else if matches!(snapshot.status, 401 | 403 | 405 | 422) {
        0.85
    } else {
        0.65
    }
}

fn infer_ai_method(snapshot: &HttpSnapshot) -> &'static str {
    if snapshot.url.contains("models") && snapshot.status != 405 {
        "GET"
    } else {
        "POST"
    }
}

fn looks_like_models_list(body: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return false;
    };
    value
        .get("data")
        .and_then(|d| d.as_array())
        .is_some_and(|arr| {
            arr.iter().any(|item| {
                item.get("id").is_some()
                    && item.get("object").and_then(|o| o.as_str()) == Some("model")
            })
        })
}

fn openai_error_shape(body: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return false;
    };
    value.get("error").is_some_and(|err| {
        err.get("message").is_some() && err.get("type").is_some()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_chat_completions_path() {
        let snapshot = HttpSnapshot {
            url: "https://example.com/v1/chat/completions".into(),
            status: 401,
            content_type: Some("application/json".into()),
            body: r#"{"error":{"message":"missing key","type":"invalid_request_error"}}"#
                .into(),
        };
        let eps = detect_ai_from_snapshot(&snapshot, None);
        assert!(eps.iter().any(|e| e.kind == EndpointKind::AiEndpoint));
    }

    #[test]
    fn detects_models_listing() {
        let snapshot = HttpSnapshot {
            url: "https://example.com/v1/models".into(),
            status: 200,
            content_type: Some("application/json".into()),
            body: r#"{"data":[{"id":"gpt-4","object":"model"}]}"#.into(),
        };
        let eps = detect_ai_from_snapshot(&snapshot, None);
        assert!(eps.len() >= 1);
    }
}
