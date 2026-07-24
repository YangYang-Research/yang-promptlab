use regex::Regex;
use std::sync::OnceLock;

use crate::types::{DiscoveredEndpoint, EndpointKind, HttpSnapshot};

fn api_path_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)/(?:api|rest|v\d+)(?:/|$)").expect("regex"))
}

pub fn detect_api_from_snapshot(
    snapshot: &HttpSnapshot,
    source_url: Option<&str>,
) -> Vec<DiscoveredEndpoint> {
    let mut found = Vec::new();

    if api_path_re().is_match(&snapshot.url) && snapshot.status != 404 {
        let confidence = if snapshot.is_json() && snapshot.is_success() {
            0.85
        } else if snapshot.is_success() {
            0.7
        } else {
            0.55
        };

        let mut ep = DiscoveredEndpoint::new(
            &snapshot.url,
            EndpointKind::RestApi,
            confidence,
            "URL path matches REST API pattern",
        )
        .with_method("GET");
        if let Some(src) = source_url {
            ep = ep.with_source(src);
        }
        found.push(ep);
    }

    if snapshot.is_success() && snapshot.is_json() && looks_like_api_catalog(&snapshot.body) {
        let mut ep = DiscoveredEndpoint::new(
            &snapshot.url,
            EndpointKind::RestApi,
            0.8,
            "JSON body resembles API catalog or HAL collection",
        )
        .with_method("GET");
        if let Some(src) = source_url {
            ep = ep.with_source(src);
        }
        found.push(ep);
    }

    found
}

fn looks_like_api_catalog(body: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return false;
    };

    if value.get("paths").is_some() {
        return false; // OpenAPI handled elsewhere
    }

    value.get("items").is_some()
        || value.get("data").is_some()
        || value.get("_links").is_some()
        || value.get("endpoints").is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_api_path() {
        let snapshot = HttpSnapshot {
            url: "https://example.com/api/v1/users".into(),
            status: 200,
            content_type: Some("application/json".into()),
            body: r#"{"data":[]}"#.into(),
        };
        let eps = detect_api_from_snapshot(&snapshot, None);
        assert!(eps.iter().any(|e| e.kind == EndpointKind::RestApi));
    }
}
