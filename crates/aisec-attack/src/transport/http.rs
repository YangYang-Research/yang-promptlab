use std::collections::HashMap;
use std::time::Instant;

use async_trait::async_trait;

use crate::error::{AttackError, AttackResult};
use crate::transport::{TargetTransport, TransportRequest, TransportResponse};

/// HTTP transport using reqwest.
pub struct HttpTransport {
    client: reqwest::Client,
}

impl HttpTransport {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::limited(5))
                .build()
                .expect("reqwest client"),
        }
    }

    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl Default for HttpTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TargetTransport for HttpTransport {
    async fn send(&self, request: TransportRequest) -> AttackResult<TransportResponse> {
        let started = Instant::now();
        let timeout = std::time::Duration::from_millis(request.timeout_ms.max(1));

        let method = reqwest::Method::from_bytes(request.method.as_bytes())
            .map_err(|e| AttackError::transport(e.to_string()))?;

        let mut builder = self
            .client
            .request(method, &request.url)
            .timeout(timeout);

        for (key, value) in &request.headers {
            builder = builder.header(key, value);
        }

        if let Some(body) = &request.body {
            builder = builder.body(body.clone());
        }

        let response = builder
            .send()
            .await
            .map_err(|e| AttackError::transport(e.to_string()))?;

        let status = response.status().as_u16();
        let headers: HashMap<String, String> = response
            .headers()
            .iter()
            .filter_map(|(k, v)| {
                v.to_str()
                    .ok()
                    .map(|val| (k.as_str().to_string(), val.to_string()))
            })
            .collect();

        let body = response
            .text()
            .await
            .map_err(|e| AttackError::transport(e.to_string()))?;

        Ok(TransportResponse {
            status,
            headers,
            body,
            duration_ms: started.elapsed().as_millis() as u64,
        })
    }
}
