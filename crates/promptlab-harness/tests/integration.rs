//! Integration tests for harness execution.

use std::sync::Arc;

use async_trait::async_trait;
use promptlab_harness::{
    AttackRequest, HarnessFactory, HarnessInterceptor, InterceptAction, NormalizedResponse,
    TargetDescriptor, TargetSurface,
};
use wiremock::matchers::{header, method, path};
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
    let descriptor = TargetDescriptor {
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
    assert_eq!(
        response.metadata.get("purpose").map(String::as_str),
        Some("attack")
    );
}

#[tokio::test]
async fn anthropic_harness_sends_messages_api_headers() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "sk-ant-test"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"content":[{"type":"text","text":"ok"}],"stop_reason":"end_turn"}"#,
        ))
        .mount(&server)
        .await;

    let factory = HarnessFactory::new().unwrap();
    let mut request = AttackRequest::from_payload(
        format!("{}/v1/messages", server.uri()),
        "hello",
    );
    request.auth.api_key = Some("sk-ant-test".into());
    let descriptor = TargetDescriptor {
        url: request.url.clone(),
        surface: TargetSurface::AnthropicCompatible,
        ..Default::default()
    };
    let response = factory.execute(&descriptor, request).await.unwrap();
    assert_eq!(response.content, "ok");
    assert_eq!(response.stop_reason.as_deref(), Some("end_turn"));
    assert_eq!(
        response.metadata.get("api_format").map(String::as_str),
        Some("anthropic_messages")
    );
}

#[tokio::test]
async fn mcp_harness_posts_existing_jsonrpc_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"pong"}]}}"#,
        ))
        .mount(&server)
        .await;

    let factory = HarnessFactory::new().unwrap();
    let mut request = AttackRequest::from_payload(format!("{}/mcp", server.uri()), "ignored");
    request.body = Some(
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"chat","arguments":{"prompt":"hi"}}}"#
            .into(),
    );
    let descriptor = TargetDescriptor {
        url: request.url.clone(),
        surface: TargetSurface::McpServer,
        ..Default::default()
    };
    let response = factory.execute(&descriptor, request).await.unwrap();
    assert!(response.content.contains("pong"));
}

#[tokio::test]
async fn interceptor_can_deny_request() {
    struct DenyAll;
    #[async_trait]
    impl HarnessInterceptor for DenyAll {
        async fn pre_execute(
            &self,
            _request: &mut AttackRequest,
        ) -> promptlab_harness::HarnessResult<InterceptAction> {
            Ok(InterceptAction::Deny {
                reason: "blocked".into(),
            })
        }
        async fn post_execute(
            &self,
            _request: &AttackRequest,
            _response: &mut NormalizedResponse,
        ) -> promptlab_harness::HarnessResult<()> {
            Ok(())
        }
    }

    let factory = HarnessFactory::new().unwrap();
    factory.add_interceptor(Arc::new(DenyAll)).unwrap();
    let err = factory
        .execute(
            &TargetDescriptor::default(),
            AttackRequest::from_payload("https://example.com", "x"),
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("blocked"));
}

#[tokio::test]
async fn redact_strips_bearer_token_from_raw() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/echo"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"response":"token was Bearer SUPERSECRETTOKENVALUE"}"#,
        ))
        .mount(&server)
        .await;

    let factory = HarnessFactory::new().unwrap();
    let mut request = AttackRequest::from_payload(format!("{}/echo", server.uri()), "x");
    request.auth.bearer_token = Some("SUPERSECRETTOKENVALUE".into());
    let descriptor = TargetDescriptor {
        url: request.url.clone(),
        ..Default::default()
    };
    let response = factory.execute(&descriptor, request).await.unwrap();
    assert!(!response.raw_response.contains("SUPERSECRETTOKENVALUE"));
    assert!(response.raw_response.contains("[REDACTED]"));
}

#[test]
fn normalized_openai_content_extraction() {
    let body = r#"{"choices":[{"message":{"content":"secret leaked"}}]}"#;
    let normalized = NormalizedResponse::from_http(200, body.into(), "openai");
    assert_eq!(normalized.content, "secret leaked");
}

#[tokio::test]
async fn chat_native_openai_sends_messages_and_model() {
    use promptlab_harness::{AttackRequest, ChatMessage, HarnessPurpose};
    use wiremock::matchers::body_partial_json;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_partial_json(serde_json::json!({
            "model": "gpt-4o-mini",
            "messages": [{"role":"user","content":"hello"}]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"choices":[{"message":{"content":"hi"},"finish_reason":"stop"}],"usage":{"prompt_tokens":3,"completion_tokens":1}}"#,
        ))
        .mount(&server)
        .await;

    let factory = HarnessFactory::new().unwrap();
    let mut request = AttackRequest::from_chat(
        format!("{}/v1/chat/completions", server.uri()),
        vec![ChatMessage::text("user", "hello")],
    );
    request.purpose = HarnessPurpose::assistant();
    request.model = Some("gpt-4o-mini".into());
    request.max_tokens = Some(32);
    let descriptor = TargetDescriptor {
        url: request.url.clone(),
        surface: TargetSurface::OpenAiCompatible,
        ..Default::default()
    };
    let response = factory.execute(&descriptor, request).await.unwrap();
    assert_eq!(response.content, "hi");
    assert_eq!(response.usage_input_tokens, Some(3));
    assert_eq!(
        response.metadata.get("purpose").map(String::as_str),
        Some("assistant")
    );
}

#[tokio::test]
async fn factory_retries_rate_limit_then_succeeds() {
    use promptlab_harness::{AttackRequest, ChatMessage, HarnessPurpose};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"choices":[{"message":{"content":"ok"}}]}"#),
        )
        .mount(&server)
        .await;

    let factory = HarnessFactory::new().unwrap();
    let mut request = AttackRequest::from_chat(
        format!("{}/v1/chat/completions", server.uri()),
        vec![ChatMessage::text("user", "ping")],
    );
    request.purpose = HarnessPurpose::assistant();
    let descriptor = TargetDescriptor {
        url: request.url.clone(),
        surface: TargetSurface::OpenAiCompatible,
        ..Default::default()
    };
    let response = factory.execute(&descriptor, request).await.unwrap();
    assert_eq!(response.content, "ok");
    assert_eq!(
        response.metadata.get("attempts").map(String::as_str),
        Some("2")
    );
}

#[tokio::test]
async fn factory_exhausted_rate_limit_is_error_for_assistant() {
    use promptlab_harness::{AttackRequest, ChatMessage, HarnessError, HarnessPurpose};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "0")
                .set_body_string(r#"{"status":429,"title":"Too Many Requests"}"#),
        )
        .mount(&server)
        .await;

    let factory = HarnessFactory::new().unwrap();
    let mut request = AttackRequest::from_chat(
        format!("{}/v1/chat/completions", server.uri()),
        vec![ChatMessage::text("user", "ping")],
    );
    request.purpose = HarnessPurpose::assistant();
    let descriptor = TargetDescriptor {
        url: request.url.clone(),
        surface: TargetSurface::OpenAiCompatible,
        ..Default::default()
    };
    let err = factory.execute(&descriptor, request).await.unwrap_err();
    assert!(matches!(err, HarnessError::RateLimited { .. }));
}

#[tokio::test]
async fn factory_exhausted_rate_limit_stays_observation_for_attack() {
    use promptlab_harness::{AttackRequest, HarnessPurpose};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "0")
                .set_body_string(r#"{"status":429,"title":"Too Many Requests"}"#),
        )
        .mount(&server)
        .await;

    let factory = HarnessFactory::new().unwrap();
    let mut request = AttackRequest::from_payload(
        format!("{}/v1/chat/completions", server.uri()),
        "probe",
    );
    request.purpose = HarnessPurpose::attack();
    let descriptor = TargetDescriptor {
        url: request.url.clone(),
        surface: TargetSurface::OpenAiCompatible,
        ..Default::default()
    };
    let response = factory.execute(&descriptor, request).await.unwrap();
    assert_eq!(response.status_code, Some(429));
}

#[tokio::test]
async fn factory_model_gone_is_error_for_assistant() {
    use promptlab_harness::{AttackRequest, ChatMessage, HarnessError, HarnessPurpose};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(410).set_body_string(
                r#"{"type":"about:blank","title":"Gone","status":410,"detail":"The model 'meta/llama-3.1-8b-instruct' has reached its end of life on 2026-08-26T09:00:00Z and is no longer available."}"#,
            ),
        )
        .mount(&server)
        .await;

    let factory = HarnessFactory::new().unwrap();
    let mut request = AttackRequest::from_chat(
        format!("{}/v1/chat/completions", server.uri()),
        vec![ChatMessage::text("user", "ping")],
    );
    request.purpose = HarnessPurpose::assistant();
    let descriptor = TargetDescriptor {
        url: request.url.clone(),
        surface: TargetSurface::OpenAiCompatible,
        ..Default::default()
    };
    let err = factory.execute(&descriptor, request).await.unwrap_err();
    match err {
        HarnessError::Http { status, message } => {
            assert_eq!(status, 410);
            assert!(message.contains("end of life"), "{message}");
        }
        other => panic!("expected Http, got {other:?}"),
    }
}
