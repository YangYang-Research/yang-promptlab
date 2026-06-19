use std::time::Instant;

use tracing::debug;

use crate::error::{AttackError, AttackResult};
use crate::transport::{TargetTransport, TransportRequest};
use crate::types::{AttackContext, AttackPayload, AttackResponse};

/// Sends payloads to the target via a transport adapter.
pub struct PayloadRunner<'a, T: TargetTransport + ?Sized> {
    transport: &'a T,
}

impl<'a, T: TargetTransport + ?Sized> PayloadRunner<'a, T> {
    pub fn new(transport: &'a T) -> Self {
        Self { transport }
    }

    pub async fn execute(
        &self,
        ctx: &AttackContext,
        payload: &AttackPayload,
        content: &str,
    ) -> AttackResult<AttackResponse> {
        let started = Instant::now();
        let request = build_request(ctx, content)?;
        let response = self
            .transport
            .send(request)
            .await
            .map_err(|e| AttackError::transport(e.to_string()))?;

        debug!(
            probe_id = %ctx.probe_id,
            payload_id = %payload.id,
            status = response.status,
            duration_ms = response.duration_ms,
            "payload executed"
        );

        Ok(AttackResponse {
            status: response.status,
            headers: response.headers,
            body: response.body,
            duration_ms: response.duration_ms.max(started.elapsed().as_millis() as u64),
            normalized: response.normalized,
        })
    }
}

fn build_request(ctx: &AttackContext, payload_content: &str) -> AttackResult<TransportRequest> {
    let target = &ctx.target;
    let method = target
        .method
        .clone()
        .unwrap_or_else(|| "POST".into());

    let body = if let Some(template) = &target.body_template {
        template.replace("{{payload}}", payload_content)
    } else {
        payload_content.to_string()
    };

    let mut headers = target.headers.clone();
    if let Some(token) = &target.auth_token {
        if !has_header(&headers, "authorization") {
            headers.insert("authorization".into(), format!("Bearer {token}"));
        }
    }
    if !has_header(&headers, "content-type") {
        headers.insert("content-type".into(), "application/json".into());
    }

    Ok(TransportRequest {
        url: target.url.clone(),
        method,
        headers,
        body: Some(body),
        timeout_ms: ctx.budget.timeout_ms,
    })
}

fn has_header(headers: &std::collections::HashMap<String, String>, name: &str) -> bool {
    headers.keys().any(|key| key.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::MockTransport;
    use crate::target_auth::apply_descriptor_auth;
    use crate::types::AttackTarget;

    #[tokio::test]
    async fn builds_json_body_from_template() {
        let transport = MockTransport::ok(r#"{"choices":[{"message":{"content":"ok"}}]}"#);
        let runner = PayloadRunner::new(&transport);
        let ctx = AttackContext::new(
            "scan-1",
            "probe-1",
            AttackTarget::llm_api("https://api.example.com/v1/chat/completions"),
        );
        let payload = AttackPayload::new("p1", "test", crate::category::AttackCategory::PromptInjection, "hello");
        let resp = runner.execute(&ctx, &payload, "hello").await.unwrap();
        assert_eq!(resp.status, 200);
    }

    #[tokio::test]
    async fn preserves_descriptor_api_key_header_on_request() {
        let transport = MockTransport::ok("{}");
        let runner = PayloadRunner::new(&transport);
        let descriptor = serde_json::json!({
            "url": "https://api.example.com/v1/chat/completions",
            "auth": {
                "kind": "api_key",
                "header": "X-API-Key",
                "value": "sk-test"
            }
        });
        let target = apply_descriptor_auth(
            AttackTarget::llm_api("https://api.example.com/v1/chat/completions"),
            &descriptor.to_string(),
        );
        let ctx = AttackContext::new("scan-1", "probe-1", target);
        let payload = AttackPayload::new("p1", "test", crate::category::AttackCategory::PromptInjection, "hello");
        runner.execute(&ctx, &payload, "hello").await.unwrap();

        let captured = transport.captured_requests();
        assert_eq!(captured.len(), 1);
        assert_eq!(
            captured[0].headers.get("X-API-Key").map(String::as_str),
            Some("sk-test")
        );
    }
}
