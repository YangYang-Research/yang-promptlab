use std::time::Duration;

use aisec_discovery::{DiscoveryConfig, DiscoveryEngine, EndpointKind};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_config() -> DiscoveryConfig {
    DiscoveryConfig {
        max_depth: 2,
        max_pages: 20,
        worker_count: 4,
        request_timeout: Duration::from_secs(5),
        allow_private_network: true,
        same_origin_only: true,
        probe_static_paths: true,
        ..Default::default()
    }
}

#[tokio::test]
async fn discovers_openapi_graphql_ai_and_crawled_links() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"<!doctype html>
            <html><body>
              <a href="/docs">Docs</a>
              <a href="/api/v1/users">Users API</a>
            </body></html>"#,
        ))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/docs"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>internal docs</html>"))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": []})))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/openapi.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "openapi": "3.1.0",
            "info": {"title": "Demo", "version": "1.0.0"},
            "paths": {}
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {"__schema": {"queryType": {"name": "Query"}}}
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_string("graphiql"))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": {"message": "missing bearer token", "type": "invalid_request_error"}
        })))
        .mount(&server)
        .await;

    let engine = DiscoveryEngine::new(test_config()).expect("engine");
    let report = engine
        .discover(&server.uri())
        .await
        .expect("discovery should succeed");

    assert!(report.stats.pages_fetched >= 1);
    assert!(report.stats.probes_sent > 0);

    assert!(
        report
            .endpoints_by_kind(EndpointKind::OpenApi)
            .iter()
            .any(|e| e.url.contains("/openapi.json")),
        "expected OpenAPI endpoint"
    );

    assert!(
        report
            .endpoints_by_kind(EndpointKind::GraphQl)
            .iter()
            .any(|e| e.url.contains("/graphql")),
        "expected GraphQL endpoint"
    );

    assert!(
        report
            .endpoints_by_kind(EndpointKind::AiEndpoint)
            .iter()
            .any(|e| e.url.contains("/v1/chat/completions")),
        "expected AI endpoint"
    );

    assert!(
        report
            .endpoints_by_kind(EndpointKind::RestApi)
            .iter()
            .any(|e| e.url.contains("/api/v1/users")),
        "expected REST API endpoint"
    );
}

#[tokio::test]
async fn rejects_private_targets_by_default() {
    let engine = DiscoveryEngine::with_defaults().expect("engine");
    let result = engine.discover("http://127.0.0.1:8080/").await;
    assert!(result.is_err());
}
