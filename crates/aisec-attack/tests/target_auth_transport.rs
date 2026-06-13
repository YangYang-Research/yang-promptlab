use aisec_attack::{
    apply_descriptor_auth, AttackCategory, AttackContext, AttackPayload, AttackTarget, HttpTransport,
    PayloadRunner,
};
use wiremock::{
    matchers::{header, method},
    Mock, MockServer, ResponseTemplate,
};

#[tokio::test]
async fn api_key_descriptor_sends_header_on_attack_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(header("X-API-Key", "sk-test"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"choices":[{"message":{"content":"ok"}}]}"#,
        ))
        .mount(&server)
        .await;

    let descriptor = serde_json::json!({
        "url": server.uri(),
        "auth": {
            "kind": "api_key",
            "header": "X-API-Key",
            "value": "sk-test"
        }
    });

    let target = apply_descriptor_auth(AttackTarget::llm_api(server.uri()), &descriptor.to_string());
    let transport = HttpTransport::new();
    let runner = PayloadRunner::new(&transport);
    let ctx = AttackContext::new("scan-1", "probe-1", target);
    let payload = AttackPayload::new("p1", "test", AttackCategory::PromptInjection, "hello");

    let response = runner.execute(&ctx, &payload, "hello").await.unwrap();
    assert_eq!(response.status, 200);
}

#[tokio::test]
async fn basic_descriptor_sends_authorization_header() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(header("Authorization", "Basic YWxpY2U6c2VjcmV0"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"choices":[{"message":{"content":"ok"}}]}"#,
        ))
        .mount(&server)
        .await;

    let descriptor = serde_json::json!({
        "url": server.uri(),
        "auth": {
            "kind": "basic",
            "username": "alice",
            "password": "secret"
        }
    });

    let target = apply_descriptor_auth(AttackTarget::llm_api(server.uri()), &descriptor.to_string());
    let transport = HttpTransport::new();
    let runner = PayloadRunner::new(&transport);
    let ctx = AttackContext::new("scan-1", "probe-1", target);
    let payload = AttackPayload::new("p1", "test", AttackCategory::PromptInjection, "hello");

    let response = runner.execute(&ctx, &payload, "hello").await.unwrap();
    assert_eq!(response.status, 200);
}
