use std::time::Instant;

use async_trait::async_trait;

use crate::factory::HarnessFactory;
use crate::models::{AttackRequest, AuthMaterial, HttpMethod, TargetDescriptor};

/// Bridges `aisec-attack` transport requests to the harness execution layer.
pub struct HarnessAttackTransport {
    factory: HarnessFactory,
    descriptor: TargetDescriptor,
    endpoint_url: String,
}

impl Clone for HarnessAttackTransport {
    fn clone(&self) -> Self {
        Self {
            factory: self.factory.clone(),
            descriptor: self.descriptor.clone(),
            endpoint_url: self.endpoint_url.clone(),
        }
    }
}

impl HarnessAttackTransport {
    pub fn new(factory: HarnessFactory, descriptor: TargetDescriptor, endpoint_url: String) -> Self {
        Self {
            factory,
            descriptor,
            endpoint_url,
        }
    }

    pub fn factory(&self) -> &HarnessFactory {
        &self.factory
    }

    pub async fn send_payload(
        &self,
        payload: &str,
        method: Option<&str>,
        headers: std::collections::HashMap<String, String>,
        body_template: Option<&str>,
        timeout_ms: u64,
    ) -> Result<TransportResponse, String> {
        let started = Instant::now();
        let mut request = AttackRequest::from_payload(&self.endpoint_url, payload);
        request.method = method
            .and_then(HttpMethod::parse)
            .unwrap_or(self.descriptor.method);
        request.headers = headers;
        request.body = body_template.map(str::to_string);
        request.timeout_ms = timeout_ms;
        request.auth = self.descriptor.auth.clone();
        request.chat_selectors = self.descriptor.chat_selectors.clone();

        let normalized = self
            .factory
            .execute(&self.descriptor, request)
            .await
            .map_err(|err| err.to_string())?;

        let raw_response = normalized.raw_response.clone();
        Ok(TransportResponse {
            status: normalized.status_code.unwrap_or(200),
            headers: normalized.metadata.clone(),
            body: raw_response,
            duration_ms: started.elapsed().as_millis() as u64,
            normalized,
        })
    }
}

/// Transport response including normalized payload for judge consumption.
#[derive(Debug, Clone)]
pub struct TransportResponse {
    pub status: u16,
    pub headers: std::collections::HashMap<String, String>,
    pub body: String,
    pub duration_ms: u64,
    pub normalized: crate::models::NormalizedResponse,
}

/// Adapter implementing attack-layer transport when `aisec-attack` is linked.
#[cfg(feature = "attack-bridge")]
pub mod attack_bridge {
    use super::*;
    use aisec_attack::{AttackResult, TargetTransport, TransportRequest, TransportResponse as AttackTransportResponse};

    pub struct AttackTransportBridge {
        inner: HarnessAttackTransport,
    }

    impl AttackTransportBridge {
        pub fn new(inner: HarnessAttackTransport) -> Self {
            Self { inner }
        }
    }

    #[async_trait]
    impl TargetTransport for AttackTransportBridge {
        async fn send(&self, request: TransportRequest) -> AttackResult<AttackTransportResponse> {
            let response = self
                .inner
                .send_payload(
                    &request.body.clone().unwrap_or_default(),
                    Some(&request.method),
                    request.headers,
                    None,
                    request.timeout_ms,
                )
                .await
                .map_err(|err| aisec_attack::AttackError::transport(err))?;

            Ok(AttackTransportResponse {
                status: response.status,
                headers: response.headers,
                body: response.body,
                duration_ms: response.duration_ms,
            })
        }
    }
}

pub fn descriptor_from_parts(
    descriptor_json: &str,
    endpoint_url: &str,
    auth: AuthMaterial,
) -> TargetDescriptor {
    let mut descriptor = TargetDescriptor::from_descriptor_json(descriptor_json)
        .unwrap_or_default();
    descriptor.url = endpoint_url.to_string();
    if !auth.headers.is_empty()
        || auth.bearer_token.is_some()
        || auth.storage_state_path.is_some()
    {
        descriptor.auth = auth;
    }
    descriptor
}
