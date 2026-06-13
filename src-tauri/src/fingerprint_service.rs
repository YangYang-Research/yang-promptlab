//! Fingerprint endpoints during discovery by probing HTTP responses.

use std::collections::HashMap;
use std::time::Duration;

use aisec_fingerprint::{FingerprintEngine, FingerprintInput, StackFingerprintReport};
use tracing::debug;

const MAX_BODY_BYTES: usize = 256 * 1024;

pub fn fingerprint_from_observation(input: &FingerprintInput) -> StackFingerprintReport {
    FingerprintEngine::new().fingerprint_stack(input)
}

pub async fn fingerprint_endpoint_url(
    client: &reqwest::Client,
    url: &str,
    method: Option<&str>,
    kind: &str,
) -> Option<StackFingerprintReport> {
    let method = method.unwrap_or("GET").to_ascii_uppercase();
    let mut request = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            url,
        )
        .timeout(Duration::from_secs(8));

    if method == "POST" {
        request = request
            .header("Content-Type", "application/json")
            .body(r#"{"model":"probe","messages":[]}"#);
    }

    let response = match request.send().await {
        Ok(resp) => resp,
        Err(err) => {
            debug!(url = %url, error = %err, "fingerprint probe failed");
            return Some(fingerprint_from_observation(&FingerprintInput {
                url: url.into(),
                method: Some(method),
                kind_hint: Some(kind.into()),
                ..Default::default()
            }));
        }
    };

    let status = response.status().as_u16();
    let headers: HashMap<String, String> = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            Some((
                name.as_str().to_string(),
                value.to_str().ok()?.to_string(),
            ))
        })
        .collect();
    let content_type = headers
        .get("content-type")
        .cloned()
        .or_else(|| headers.get("Content-Type").cloned());
    let bytes = response.bytes().await.ok()?;
    let body = if bytes.len() > MAX_BODY_BYTES {
        String::from_utf8_lossy(&bytes[..MAX_BODY_BYTES]).into_owned()
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    };

    Some(fingerprint_from_observation(&FingerprintInput {
        url: url.into(),
        method: Some(method),
        status: Some(status),
        headers,
        body: Some(body),
        content_type,
        kind_hint: Some(kind.into()),
    }))
}

pub fn should_fingerprint_kind(kind: &str) -> bool {
    matches!(
        kind,
        "ai_endpoint" | "openapi" | "graphql" | "javascript" | "rest_api"
    )
}

pub fn fingerprint_json(report: &StackFingerprintReport) -> String {
    serde_json::to_string(report).unwrap_or_else(|_| "{}".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprints_url_only_input() {
        let report = fingerprint_from_observation(&FingerprintInput {
            url: "https://api.openai.com/v1/chat/completions".into(),
            method: Some("POST".into()),
            status: Some(401),
            body: Some(r#"{"error":{"type":"invalid_request_error"}}"#.into()),
            ..Default::default()
        });
        assert!(report.confidence > 0.0);
        assert!(!report.technologies.is_empty());
    }
}
