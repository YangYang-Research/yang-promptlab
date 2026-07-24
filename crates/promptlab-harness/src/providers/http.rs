use std::time::{Duration, Instant};

use async_trait::async_trait;
use reqwest::Client;
use tracing::debug;

use crate::error::{HarnessError, HarnessResult};
use crate::models::{AttackRequest, NormalizedResponse};
use crate::traits::Harness;

/// Generic HTTP harness for REST and MCP-over-HTTP targets.
#[derive(Clone)]
pub struct HttpHarness {
    client: Client,
}

impl HttpHarness {
    pub fn new() -> HarnessResult<Self> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(HarnessError::from)?;
        Ok(Self { client })
    }
}

impl Default for HttpHarness {
    fn default() -> Self {
        Self::new().expect("http harness client")
    }
}

#[async_trait]
impl Harness for HttpHarness {
    fn id(&self) -> &'static str {
        "http"
    }

    async fn execute(&self, request: AttackRequest) -> HarnessResult<NormalizedResponse> {
        let started = Instant::now();
        let headers = request.merged_headers();
        let method = request.method.as_str();
        let body = request.effective_body();

        let mut builder = self
            .client
            .request(
                reqwest::Method::from_bytes(method.as_bytes())
                    .map_err(|e| HarnessError::config(e.to_string()))?,
                &request.url,
            )
            .timeout(Duration::from_millis(request.timeout_ms.max(1)));

        for (key, value) in headers {
            builder = builder.header(key, value);
        }

        if !matches!(request.method, crate::models::HttpMethod::Get) {
            builder = builder.body(body);
        }

        let response = builder.send().await?;
        let status = response.status().as_u16();
        let raw = response.text().await.unwrap_or_default();

        debug!(
            harness = "http",
            url = %request.url,
            status,
            duration_ms = started.elapsed().as_millis(),
            "harness execute complete"
        );

        Ok(NormalizedResponse::from_http(status, raw, self.id()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn executes_post_request() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"response":"pong"}"#))
            .mount(&server)
            .await;

        let harness = HttpHarness::new().unwrap();
        let response = harness
            .execute(AttackRequest::from_payload(
                format!("{}/v1/chat", server.uri()),
                "ping",
            ))
            .await
            .unwrap();

        assert_eq!(response.status_code, Some(200));
        assert!(response.content.contains("pong"));
    }
}
