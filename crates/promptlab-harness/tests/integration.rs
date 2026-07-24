//! Integration tests for harness execution.

use promptlab_harness::{AttackRequest, HarnessFactory, NormalizedResponse};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn factory_executes_http_target() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"response":"hello"}"#))
        .mount(&server)
        .await;

    let factory = HarnessFactory::new().unwrap();
    let descriptor = promptlab_harness::TargetDescriptor {
        url: format!("{}/api/chat", server.uri()),
        ..Default::default()
    };

    let response = factory
        .execute(
            &descriptor,
            AttackRequest::from_payload(descriptor.url.clone(), "probe"),
        )
        .await
        .unwrap();

    assert_eq!(response.status_code, Some(200));
    assert!(response.content.contains("hello"));
}

#[test]
fn normalized_openai_content_extraction() {
    let body = r#"{"choices":[{"message":{"content":"secret leaked"}}]}"#;
    let normalized = NormalizedResponse::from_http(200, body.into(), "openai");
    assert_eq!(normalized.content, "secret leaked");
}
