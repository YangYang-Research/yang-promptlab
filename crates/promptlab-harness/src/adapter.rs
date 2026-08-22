use std::time::Instant;

use crate::cancel::CancelFlag;
use crate::factory::HarnessFactory;
use crate::models::{AttackRequest, AuthMaterial, HttpMethod, TargetDescriptor};

/// Bridges `promptlab-attack` transport requests to the harness execution layer.
pub struct HarnessAttackTransport {
    factory: HarnessFactory,
    descriptor: TargetDescriptor,
    endpoint_url: String,
    cancel: CancelFlag,
}

impl Clone for HarnessAttackTransport {
    fn clone(&self) -> Self {
        Self {
            factory: self.factory.clone(),
            descriptor: self.descriptor.clone(),
            endpoint_url: self.endpoint_url.clone(),
            cancel: self.cancel.clone(),
        }
    }
}

impl HarnessAttackTransport {
    pub fn new(factory: HarnessFactory, descriptor: TargetDescriptor, endpoint_url: String) -> Self {
        Self {
            factory,
            descriptor,
            endpoint_url,
            cancel: CancelFlag::new(),
        }
    }

    pub fn with_cancel(mut self, cancel: CancelFlag) -> Self {
        self.cancel = cancel;
        self
    }

    pub fn factory(&self) -> &HarnessFactory {
        &self.factory
    }

    pub fn descriptor(&self) -> &TargetDescriptor {
        &self.descriptor
    }

    pub fn endpoint_url(&self) -> &str {
        &self.endpoint_url
    }

    pub fn cancel_flag(&self) -> &CancelFlag {
        &self.cancel
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
        request.stream = self.descriptor.stream;
        request.conversation_id = self.descriptor.conversation_id.clone();
        request.mcp_method = self.descriptor.mcp_method.clone();
        request.mcp_session_id = self.descriptor.mcp_session_id.clone();
        request.ws_subprotocol = self.descriptor.ws_subprotocol.clone();
        request.cancel = self.cancel.clone();

        let normalized = self
            .factory
            .execute(&self.descriptor, request)
            .await
            .map_err(|err| err.to_string())?;

        let raw_response = normalized.raw_response.clone();
        Ok(TransportResponse {
            status: normalized.status_code.unwrap_or(200),
            headers: normalized.headers.clone(),
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

pub fn descriptor_from_parts(
    descriptor_json: &str,
    endpoint_url: &str,
    auth: AuthMaterial,
) -> TargetDescriptor {
    let mut descriptor = TargetDescriptor::from_descriptor_json(descriptor_json)
        .unwrap_or_default();
    descriptor.url = endpoint_url.to_string();
    merge_auth(&mut descriptor.auth, auth);
    descriptor
}

fn merge_auth(dst: &mut AuthMaterial, src: AuthMaterial) {
    dst.headers.extend(src.headers);
    if src.bearer_token.is_some() {
        dst.bearer_token = src.bearer_token;
    }
    if src.basic_username.is_some() {
        dst.basic_username = src.basic_username;
    }
    if src.basic_password.is_some() {
        dst.basic_password = src.basic_password;
    }
    if src.cookie_header.is_some() {
        dst.cookie_header = src.cookie_header;
    }
    if src.storage_state_path.is_some() {
        dst.storage_state_path = src.storage_state_path;
    }
    if src.api_key_header.is_some() {
        dst.api_key_header = src.api_key_header;
    }
    if src.api_key.is_some() {
        dst.api_key = src.api_key;
    }
    if src.query_key_name.is_some() {
        dst.query_key_name = src.query_key_name;
    }
    if src.query_key_value.is_some() {
        dst.query_key_value = src.query_key_value;
    }
    if src.aws_access_key_id.is_some() {
        dst.aws_access_key_id = src.aws_access_key_id;
    }
    if src.aws_secret_access_key.is_some() {
        dst.aws_secret_access_key = src.aws_secret_access_key;
    }
    if src.aws_session_token.is_some() {
        dst.aws_session_token = src.aws_session_token;
    }
    if src.aws_region.is_some() {
        dst.aws_region = src.aws_region;
    }
    if src.aws_service.is_some() {
        dst.aws_service = src.aws_service;
    }
}
