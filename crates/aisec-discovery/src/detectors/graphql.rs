use crate::types::{DiscoveredEndpoint, EndpointKind, HttpSnapshot};

const INTROSPECTION_QUERY: &str =
    r#"{"query":"query IntrospectionQuery { __schema { queryType { name } } }"}"#;

pub fn detect_graphql_from_snapshot(
    snapshot: &HttpSnapshot,
    source_url: Option<&str>,
) -> Vec<DiscoveredEndpoint> {
    let mut found = Vec::new();

    if snapshot.is_success() && is_graphql_json_response(&snapshot.body) {
        let mut ep = DiscoveredEndpoint::new(
            &snapshot.url,
            EndpointKind::GraphQl,
            0.95,
            "GraphQL introspection response detected",
        )
        .with_method("POST");
        if let Some(src) = source_url {
            ep = ep.with_source(src);
        }
        found.push(ep);
    }

    if snapshot.is_success() && is_graphql_playground_html(&snapshot.body) {
        let mut ep = DiscoveredEndpoint::new(
            &snapshot.url,
            EndpointKind::GraphQl,
            0.75,
            "GraphQL playground UI markers in HTML",
        )
        .with_method("GET");
        if let Some(src) = source_url {
            ep = ep.with_source(src);
        }
        found.push(ep);
    }

    if snapshot.url.to_lowercase().contains("graphql") && snapshot.status != 404 {
        let confidence = if snapshot.is_success() { 0.7 } else { 0.55 };
        let mut ep = DiscoveredEndpoint::new(
            &snapshot.url,
            EndpointKind::GraphQl,
            confidence,
            "URL path suggests GraphQL endpoint",
        );
        if snapshot.status == 405 {
            ep = ep.with_method("POST");
        }
        if let Some(src) = source_url {
            ep = ep.with_source(src);
        }
        found.push(ep);
    }

    found
}

pub async fn probe_graphql_paths(
    client: &crate::client::HttpClient,
    origin: &str,
) -> Vec<DiscoveredEndpoint> {
    use crate::detectors::paths::graphql_probe_paths;

    let mut found = Vec::new();
    for url in graphql_probe_paths(origin) {
        if let Ok(snapshot) = client.post_json(&url, INTROSPECTION_QUERY).await {
            if !snapshot.body.is_empty() {
                found.extend(detect_graphql_from_snapshot(&snapshot, Some(origin)));
                continue;
            }
        }
        if let Ok(snapshot) = client.get(&url).await {
            found.extend(detect_graphql_from_snapshot(&snapshot, Some(origin)));
        }
    }
    found
}

fn is_graphql_json_response(body: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return false;
    };
    value
        .pointer("/data/__schema")
        .is_some()
        || value
            .get("data")
            .and_then(|d| d.get("__schema"))
            .is_some()
}

fn is_graphql_playground_html(body: &str) -> bool {
    let lower = body.to_lowercase();
    lower.contains("graphiql")
        || lower.contains("graphql playground")
        || lower.contains("apollo sandbox")
        || lower.contains("altair")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_introspection_json() {
        let snapshot = HttpSnapshot {
            url: "https://example.com/graphql".into(),
            status: 200,
            content_type: Some("application/json".into()),
            body: r#"{"data":{"__schema":{"queryType":{"name":"Query"}}}}"#.into(),
        };
        let eps = detect_graphql_from_snapshot(&snapshot, None);
        assert!(eps.iter().any(|e| e.confidence >= 0.9));
    }
}
