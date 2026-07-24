use crate::types::{DiscoveredEndpoint, EndpointKind, HttpSnapshot};

pub fn detect_openapi_from_snapshot(
    snapshot: &HttpSnapshot,
    source_url: Option<&str>,
) -> Vec<DiscoveredEndpoint> {
    if !snapshot.is_success() {
        return Vec::new();
    }

    let body = snapshot.body.trim();
    if body.is_empty() {
        return Vec::new();
    }

    if is_openapi_json(body) {
        let mut ep = DiscoveredEndpoint::new(
            &snapshot.url,
            EndpointKind::OpenApi,
            0.95,
            "response body contains OpenAPI/Swagger JSON",
        )
        .with_method("GET");
        if let Some(src) = source_url {
            ep = ep.with_source(src);
        }
        return vec![ep];
    }

    if is_openapi_yaml(body) {
        let mut ep = DiscoveredEndpoint::new(
            &snapshot.url,
            EndpointKind::OpenApi,
            0.9,
            "response body contains OpenAPI YAML marker",
        )
        .with_method("GET");
        if let Some(src) = source_url {
            ep = ep.with_source(src);
        }
        return vec![ep];
    }

    Vec::new()
}

pub async fn probe_openapi_paths(
    client: &crate::client::HttpClient,
    origin: &str,
) -> Vec<DiscoveredEndpoint> {
    use crate::detectors::paths::openapi_probe_paths;

    let mut found = Vec::new();
    for url in openapi_probe_paths(origin) {
        match client.get(&url).await {
            Ok(snapshot) => {
                found.extend(detect_openapi_from_snapshot(&snapshot, Some(origin)));
            }
            Err(_) => continue,
        }
    }
    found
}

fn is_openapi_json(body: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return false;
    };
    value.get("openapi").is_some() || value.get("swagger").is_some()
}

fn is_openapi_yaml(body: &str) -> bool {
    let lower = body.to_lowercase();
    lower.contains("openapi:") || lower.contains("swagger:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_openapi_json() {
        let snapshot = HttpSnapshot {
            url: "https://example.com/openapi.json".into(),
            status: 200,
            content_type: Some("application/json".into()),
            body: r#"{"openapi":"3.1.0","paths":{}}"#.into(),
        };
        let eps = detect_openapi_from_snapshot(&snapshot, None);
        assert_eq!(eps.len(), 1);
        assert_eq!(eps[0].kind, EndpointKind::OpenApi);
    }
}
