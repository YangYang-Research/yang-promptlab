use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures_util::StreamExt;
use promptlab_core::{build_http_client, current_proxy_settings, HttpClientOptions, ProxySettings};
use reqwest::Client;
use tracing::debug;

use crate::error::{HarnessError, HarnessResult};
use crate::models::{AttackRequest, HttpMethod, NormalizedResponse};
use crate::sse::{consume_sse_chunk, request_wants_stream};
use crate::traits::Harness;

/// Generic HTTP harness for REST targets.
#[derive(Clone)]
pub struct HttpHarness {
    /// Rebuilds when Settings → Proxy changes so attack traffic always follows current policy.
    cache: Arc<Mutex<Option<(ProxySettings, Client)>>>,
}

impl HttpHarness {
    pub fn new() -> HarnessResult<Self> {
        Ok(Self {
            cache: Arc::new(Mutex::new(None)),
        })
    }

    pub fn client(&self) -> HarnessResult<Client> {
        let current = current_proxy_settings();
        let mut guard = self
            .cache
            .lock()
            .map_err(|_| HarnessError::config("http harness proxy cache poisoned"))?;
        if let Some((settings, client)) = guard.as_ref() {
            if settings == &current {
                return Ok(client.clone());
            }
        }
        let client = build_http_client(HttpClientOptions::default().with_redirect_limit(5))
            .map_err(|e| HarnessError::config(e.to_string()))?;
        *guard = Some((current, client.clone()));
        Ok(client)
    }

    pub async fn execute_raw(
        &self,
        request: &AttackRequest,
        harness_id: &str,
    ) -> HarnessResult<NormalizedResponse> {
        request.cancel.check()?;
        let started = Instant::now();
        let url = request.auth.apply_query_key(&request.url);
        let headers = request.merged_headers();
        let method = request.method.as_str();
        let body = request.effective_body();
        let client = self.client()?;
        let stream = request_wants_stream(request.body.as_deref(), request.stream);

        let mut builder = client
            .request(
                reqwest::Method::from_bytes(method.as_bytes())
                    .map_err(|e| HarnessError::config(e.to_string()))?,
                &url,
            )
            .timeout(Duration::from_millis(request.timeout_ms.max(1)));

        for (key, value) in &headers {
            builder = builder.header(key, value);
        }
        if stream && !headers.keys().any(|k| k.eq_ignore_ascii_case("accept")) {
            builder = builder.header("Accept", "text/event-stream");
        }
        if !matches!(request.method, HttpMethod::Get) {
            builder = builder.body(body);
        }

        let response = builder.send().await?;
        request.cancel.check()?;
        let status = response.status().as_u16();
        let response_headers = collect_response_headers(response.headers());
        let content_type = response_headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.as_str())
            .unwrap_or("");
        let sse = stream || content_type.contains("text/event-stream");

        let raw = if sse {
            read_sse_body(response, request).await?
        } else {
            response.text().await.unwrap_or_default()
        };

        debug!(
            harness = harness_id,
            url = %url,
            status,
            duration_ms = started.elapsed().as_millis(),
            "harness execute complete"
        );

        let mut normalized = NormalizedResponse::from_http_headers(
            status,
            response_headers,
            raw,
            harness_id,
        );
        if let Some(session) = response_session_header(&normalized) {
            normalized
                .metadata
                .insert("mcp_session_id".into(), session);
        }
        Ok(normalized)
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
        self.execute_raw(&request, self.id()).await
    }
}

fn response_session_header(response: &NormalizedResponse) -> Option<String> {
    response
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("mcp-session-id"))
        .map(|(_, v)| v.clone())
}

async fn read_sse_body(
    response: reqwest::Response,
    request: &AttackRequest,
) -> HarnessResult<String> {
    let mut buffer = String::new();
    let mut assembled = String::new();
    let mut raw = String::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        request.cancel.check()?;
        let bytes = chunk.map_err(HarnessError::from)?;
        let text = String::from_utf8_lossy(&bytes);
        raw.push_str(&text);
        let before = assembled.len();
        consume_sse_chunk(&mut buffer, &text, &mut assembled);
        if assembled.len() > before {
            request.emit_stream(&assembled[before..]);
        }
    }
    if assembled.trim().is_empty() {
        Ok(raw)
    } else {
        Ok(serde_json::json!({ "answer": assembled, "raw_sse": raw }).to_string())
    }
}

fn collect_response_headers(map: &reqwest::header::HeaderMap) -> std::collections::HashMap<String, String> {
    let mut headers = std::collections::HashMap::new();
    for (name, value) in map.iter() {
        let key = name.as_str().to_string();
        let val = match value.to_str() {
            Ok(text) => text.to_string(),
            Err(_) => String::from_utf8_lossy(value.as_bytes()).into_owned(),
        };
        headers
            .entry(key)
            .and_modify(|existing: &mut String| {
                existing.push_str(", ");
                existing.push_str(&val);
            })
            .or_insert(val);
    }
    headers
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
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"response":"pong"}"#)
                    .insert_header("Content-Type", "application/json; charset=utf-8")
                    .insert_header("Server", "llama.cpp"),
            )
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
        assert!(
            response
                .headers
                .keys()
                .any(|key| key.eq_ignore_ascii_case("content-type")),
            "expected Content-Type in {:?}",
            response.headers
        );
        assert_eq!(
            response.headers.get("server").map(String::as_str),
            Some("llama.cpp")
        );
        assert!(!response.headers.contains_key("harness"));
        assert_eq!(
            response.metadata.get("harness").map(String::as_str),
            Some("http")
        );
    }
}
