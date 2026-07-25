use std::collections::HashMap;
use std::sync::Arc;

use promptlab_core::{PromptLabError, PromptLabResult};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, USER_AGENT};
use reqwest::{Client, Method, Response, StatusCode};
use tracing::instrument;

use crate::config::DiscoveryConfig;
use crate::retry::{is_retryable_status, with_reqwest_retry};
use crate::types::HttpSnapshot;

/// Shared HTTP client with timeouts, size limits, and retry semantics.
#[derive(Clone)]
pub struct HttpClient {
    inner: Client,
    config: Arc<DiscoveryConfig>,
    auth_headers: HashMap<String, String>,
}

impl HttpClient {
    pub fn new(config: DiscoveryConfig) -> PromptLabResult<Self> {
        let config = Arc::new(config);
        let inner = promptlab_core::build_http_client(
            promptlab_core::HttpClientOptions::default()
                .with_user_agent(config.user_agent.clone())
                .with_timeout(config.request_timeout)
                .with_redirect_limit(5),
        )?;

        Ok(Self {
            inner,
            config,
            auth_headers: HashMap::new(),
        })
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
        self.request(Method::GET, url, None).await
    }

    #[instrument(skip(self, body), fields(url = %url))]
    pub async fn post_json(&self, url: &str, body: &str) -> PromptLabResult<HttpSnapshot> {
        self.request(Method::POST, url, Some(body)).await
    }

    async fn request(
        &self,
        method: Method,
        url: &str,
        json_body: Option<&str>,
    ) -> PromptLabResult<HttpSnapshot> {
        let label = format!("{method} {url}");
        let config = self.config.clone();
        let client = self.inner.clone();
        let auth_headers = self.auth_headers.clone();
        let method_clone = method.clone();
        let url = url.to_string();
        let body = json_body.map(str::to_string);

        let response = with_reqwest_retry(&label, &config.retry, || {
            let client = client.clone();
            let method = method_clone.clone();
            let url = url.clone();
            let body = body.clone();
            let auth_headers = auth_headers.clone();
            async move {
                let mut req = client.request(method, &url);
                if let Some(payload) = body {
                    req = req
                        .header("Content-Type", "application/json")
                        .body(payload);
                }
                for (key, value) in &auth_headers {
                    if let (Ok(name), Ok(val)) = (
                        HeaderName::from_bytes(key.as_bytes()),
                        HeaderValue::from_str(value),
                    ) {
                        req = req.header(name, val);
                    }
                }
                req.send().await
            }
        })
        .await
        .map_err(|err| PromptLabError::internal(format!("HTTP request failed: {err}")))?;

        let status = response.status();
        if is_retryable_status(status.as_u16()) {
            return Err(PromptLabError::internal(format!(
                "HTTP {} after retries",
                status.as_u16()
            )));
        }

        self.snapshot(response).await
    }

    async fn snapshot(&self, response: Response) -> PromptLabResult<HttpSnapshot> {
        let url = response.url().to_string();
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);

        let bytes = response
            .bytes()
            .await
            .map_err(|err| PromptLabError::internal(format!("failed to read body: {err}")))?;

        if bytes.len() > self.config.max_body_bytes {
            return Err(PromptLabError::invalid_input(format!(
                "response body exceeds max_body_bytes ({})",
                self.config.max_body_bytes
            )));
        }

        let body = String::from_utf8_lossy(&bytes).into_owned();

        Ok(HttpSnapshot {
            url,
            status,
            content_type,
            body,
        })
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
