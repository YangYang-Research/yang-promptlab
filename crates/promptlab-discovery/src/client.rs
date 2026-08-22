use std::collections::HashMap;
use std::sync::Arc;

use promptlab_core::{PromptLabError, PromptLabResult};
use promptlab_harness::{
    HarnessFactory, HarnessPurpose, HarnessRequest, HttpMethod, TargetDescriptor, TargetSurface,
};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use reqwest::StatusCode;
use tracing::instrument;

use crate::config::DiscoveryConfig;
use crate::retry::{is_retryable_status, with_retry};
use crate::types::HttpSnapshot;

/// Shared HTTP client — target I/O goes through [`HarnessFactory`], not a private reqwest stack.
#[derive(Clone)]
pub struct HttpClient {
    factory: HarnessFactory,
    config: Arc<DiscoveryConfig>,
    auth_headers: HashMap<String, String>,
}

impl HttpClient {
    pub fn new(config: DiscoveryConfig) -> PromptLabResult<Self> {
        let factory = HarnessFactory::new()
            .map_err(|err| PromptLabError::internal(format!("harness factory: {err}")))?;
        Ok(Self {
            factory,
            config: Arc::new(config),
            auth_headers: HashMap::new(),
        })
    }

    pub fn with_factory(mut self, factory: HarnessFactory) -> Self {
        self.factory = factory;
        self
    }

    pub fn with_auth_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.auth_headers = headers;
        self
    }

    pub fn config(&self) -> &DiscoveryConfig {
        &self.config
    }

    #[instrument(skip(self), fields(url = %url))]
    pub async fn get(&self, url: &str) -> PromptLabResult<HttpSnapshot> {
        self.request(HttpMethod::Get, url, None).await
    }

    #[instrument(skip(self, body), fields(url = %url))]
    pub async fn post_json(&self, url: &str, body: &str) -> PromptLabResult<HttpSnapshot> {
        self.request(HttpMethod::Post, url, Some(body)).await
    }

    async fn request(
        &self,
        method: HttpMethod,
        url: &str,
        json_body: Option<&str>,
    ) -> PromptLabResult<HttpSnapshot> {
        let label = format!("{} {url}", method.as_str());
        let config = self.config.clone();
        let factory = self.factory.clone();
        let auth_headers = self.auth_headers.clone();
        let user_agent = self.config.user_agent.clone();
        let url = url.to_string();
        let body = json_body.map(str::to_string);
        let timeout_ms = config.request_timeout.as_millis().max(1) as u64;
        let max_body_bytes = config.max_body_bytes;

        let snapshot = with_retry(&label, &config.retry, || {
            let factory = factory.clone();
            let url = url.clone();
            let body = body.clone();
            let auth_headers = auth_headers.clone();
            let user_agent = user_agent.clone();
            async move {
                let mut request = HarnessRequest::from_payload(&url, body.clone().unwrap_or_default())
                    .with_purpose(HarnessPurpose::discover());
                request.method = method;
                request.body = body;
                request.timeout_ms = timeout_ms;
                request.headers = auth_headers;
                if !request
                    .headers
                    .keys()
                    .any(|k| k.eq_ignore_ascii_case("user-agent"))
                {
                    request.headers.insert("User-Agent".into(), user_agent);
                }
                let descriptor = TargetDescriptor {
                    url: url.clone(),
                    surface: TargetSurface::RestApi,
                    method,
                    ..TargetDescriptor::default()
                };
                let normalized = factory
                    .execute(&descriptor, request)
                    .await
                    .map_err(|err| PromptLabError::internal(format!("HTTP request failed: {err}")))?;
                let status = normalized.status_code.unwrap_or(0);
                if is_retryable_status(status) {
                    return Err(PromptLabError::internal(format!(
                        "HTTP {status} after retries"
                    )));
                }
                if normalized.raw_response.len() > max_body_bytes {
                    return Err(PromptLabError::invalid_input(format!(
                        "response body exceeds max_body_bytes ({max_body_bytes})"
                    )));
                }
                let content_type = normalized
                    .headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                    .map(|(_, v)| v.clone());
                Ok(HttpSnapshot {
                    url,
                    status,
                    content_type,
                    body: normalized.raw_response,
                })
            }
        })
        .await?;

        Ok(snapshot)
    }

    pub fn default_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&self.config.user_agent)
                .unwrap_or_else(|_| HeaderValue::from_static("PromptLab-Discovery")),
        );
        headers
    }
}

impl HttpSnapshot {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn is_json(&self) -> bool {
        self.content_type
            .as_deref()
            .is_some_and(|ct| ct.contains("json"))
    }

    pub fn status_code(&self) -> StatusCode {
        StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_client_with_defaults() {
        let client = HttpClient::new(DiscoveryConfig::default()).expect("client");
        assert_eq!(client.config().worker_count, 8);
    }
}
