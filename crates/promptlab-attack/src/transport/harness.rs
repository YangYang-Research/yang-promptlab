use async_trait::async_trait;
use promptlab_harness::{
    adapter::descriptor_from_parts, AuthMaterial, HarnessAttackTransport, HarnessFactory,
    HarnessKind, HttpMethod, TargetDescriptor, TargetSurface,
};

use crate::error::{AttackError, AttackResult};
use crate::transport::{TargetTransport, TransportRequest, TransportResponse};
use crate::types::{AttackTarget, TargetKind};

/// Production transport: routes all attack payloads through the harness layer.
#[derive(Clone)]
pub struct HarnessTransport {
    inner: HarnessAttackTransport,
}

impl HarnessTransport {
    pub fn new(inner: HarnessAttackTransport) -> Self {
        Self { inner }
    }

    pub fn from_parts(
        factory: HarnessFactory,
        descriptor: TargetDescriptor,
        endpoint_url: impl Into<String>,
    ) -> Self {
        Self::new(HarnessAttackTransport::new(
            factory,
            descriptor,
            endpoint_url.into(),
        ))
    }

    /// Build a harness transport for a single attack target (HTTP/OpenAI surfaces).
    pub fn for_attack_target(target: &AttackTarget) -> AttackResult<Self> {
        let factory = HarnessFactory::new().map_err(|err| AttackError::transport(err.to_string()))?;
        let descriptor = attack_target_to_descriptor(target);
        Ok(Self::from_parts(factory, descriptor, target.url.clone()))
    }

    /// Build from persisted target descriptor JSON plus optional auth material.
    pub fn from_descriptor_json(
        descriptor_json: &str,
        endpoint_url: &str,
        auth: AuthMaterial,
    ) -> AttackResult<Self> {
        let factory = HarnessFactory::new().map_err(|err| AttackError::transport(err.to_string()))?;
        let descriptor = descriptor_from_parts(descriptor_json, endpoint_url, auth);
        Ok(Self::from_parts(factory, descriptor, endpoint_url.to_string()))
    }

    pub fn inner(&self) -> &HarnessAttackTransport {
        &self.inner
    }

    pub fn into_inner(self) -> HarnessAttackTransport {
        self.inner
    }

    pub fn factory(&self) -> &HarnessFactory {
        self.inner.factory()
    }

    pub fn descriptor(&self) -> &TargetDescriptor {
        self.inner.descriptor()
    }

    pub fn preferred_harness(&self) -> HarnessKind {
        self.inner.descriptor().preferred_harness()
    }

    pub fn with_factory(mut self, factory: HarnessFactory) -> Self {
        let endpoint = self.inner.endpoint_url().to_string();
        let descriptor = self.inner.descriptor().clone();
        let cancel = self.inner.cancel_flag().clone();
        self.inner = HarnessAttackTransport::new(factory, descriptor, endpoint).with_cancel(cancel);
        self
    }

    pub fn with_cancel(mut self, cancel: promptlab_harness::CancelFlag) -> Self {
        self.inner = self.inner.with_cancel(cancel);
        self
    }
}

#[async_trait]
impl TargetTransport for HarnessTransport {
    async fn send(&self, request: TransportRequest) -> AttackResult<TransportResponse> {
        // Pre-built profile body must be sent as-is (same as verification step 2).
        // Passing it as `payload` with no body would re-wrap it in a default OpenAI schema.
        let response = self
            .inner
            .send_payload(
                "",
                Some(&request.method),
                request.headers,
                request.body.as_deref(),
                request.timeout_ms,
            )
            .await
            .map_err(|err| AttackError::transport(err))?;

        Ok(TransportResponse {
            status: response.status,
            headers: response.headers,
            body: response.body,
            duration_ms: response.duration_ms,
            normalized: response.normalized,
        })
    }
}

fn attack_target_to_descriptor(target: &AttackTarget) -> TargetDescriptor {
    let surface = target
        .harness_surface
        .as_deref()
        .and_then(TargetSurface::parse)
        .unwrap_or_else(|| match target.kind {
            TargetKind::LlmApi => TargetSurface::OpenAiCompatible,
            TargetKind::Chatbot => TargetSurface::BrowserChat,
            TargetKind::Mcp => TargetSurface::McpServer,
            TargetKind::Agent | TargetKind::Rag => TargetSurface::RestApi,
        });

    let method = target
        .method
        .as_deref()
        .and_then(HttpMethod::parse)
        .unwrap_or(HttpMethod::Post);

    let mut auth = AuthMaterial::default();
    if let Some(token) = &target.auth_token {
        auth.bearer_token = Some(token.clone());
    }

    TargetDescriptor {
        url: target.url.clone(),
        surface,
        method,
        headers: target.headers.clone(),
        body_template: target.body_template.clone(),
        auth,
        ..TargetDescriptor::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::MockTransport;
    use crate::types::AttackContext;

    #[test]
    fn generic_http_profile_uses_http_harness() {
        let mut target = AttackTarget::llm_api("https://api.example.com/v1/custom");
        target.harness_surface = Some("rest_api".into());
        let transport = HarnessTransport::for_attack_target(&target).unwrap();
        assert_eq!(transport.preferred_harness(), HarnessKind::Http);
    }

    #[test]
    fn maps_llm_api_to_openai_harness() {
        let target = AttackTarget::llm_api("https://api.example.com/v1/chat/completions");
        let transport = HarnessTransport::for_attack_target(&target).unwrap();
        assert_eq!(transport.preferred_harness(), HarnessKind::OpenAi);
    }

    #[test]
    fn mock_transport_still_usable_in_unit_tests() {
        let _mock = MockTransport::ok("{}");
        let _ctx = AttackContext::new("s", "p", AttackTarget::llm_api("https://example.com"));
    }
}
