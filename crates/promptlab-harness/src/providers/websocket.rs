use std::time::Duration;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

use crate::error::{HarnessError, HarnessResult};
use crate::models::{AttackRequest, NormalizedResponse};
use crate::traits::Harness;

/// Generic WebSocket text-frame harness.
#[derive(Clone, Default)]
pub struct WebSocketHarness;

impl WebSocketHarness {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Harness for WebSocketHarness {
    fn id(&self) -> &'static str {
        "websocket"
    }

    async fn execute(&self, request: AttackRequest) -> HarnessResult<NormalizedResponse> {
        request.cancel.check()?;
        let timeout = Duration::from_millis(request.timeout_ms.max(1));
        let outcome = tokio::time::timeout(timeout, exchange(&request)).await;
        match outcome {
            Ok(Ok(text)) => {
                request.emit_stream(&text);
                let mut response = NormalizedResponse::from_http(200, text, self.id());
                response
                    .metadata
                    .insert("transport".into(), "websocket".into());
                Ok(response)
            }
            Ok(Err(err)) => Err(err),
            Err(_) => Err(HarnessError::transport("websocket timed out")),
        }
    }
}

async fn exchange(request: &AttackRequest) -> HarnessResult<String> {
    let mut ws_url = request.url.clone();
    if let Some(http) = ws_url.strip_prefix("https://") {
        ws_url = format!("wss://{http}");
    } else if let Some(http) = ws_url.strip_prefix("http://") {
        ws_url = format!("ws://{http}");
    }
    let mut req = ws_url
        .as_str()
        .into_client_request()
        .map_err(|err| HarnessError::config(err.to_string()))?;
    if let Some(proto) = &request.ws_subprotocol {
        req.headers_mut().insert(
            "Sec-WebSocket-Protocol",
            proto
                .parse()
                .map_err(|err| HarnessError::config(format!("ws subprotocol: {err}")))?,
        );
    }
    for (key, value) in request.merged_headers() {
        if key.eq_ignore_ascii_case("content-type") {
            continue;
        }
        if let (Ok(name), Ok(val)) = (
            tokio_tungstenite::tungstenite::http::HeaderName::from_bytes(key.as_bytes()),
            value.parse::<tokio_tungstenite::tungstenite::http::HeaderValue>(),
        ) {
            req.headers_mut().insert(name, val);
        }
    }

    let (mut socket, _response) = tokio_tungstenite::connect_async(req)
        .await
        .map_err(|err| HarnessError::transport(err.to_string()))?;
    request.cancel.check()?;
    let payload = request.effective_body();
    socket
        .send(Message::Text(payload.into()))
        .await
        .map_err(|err| HarnessError::transport(err.to_string()))?;

    while let Some(message) = socket.next().await {
        request.cancel.check()?;
        match message.map_err(|err| HarnessError::transport(err.to_string()))? {
            Message::Text(text) => {
                let _ = socket.close(None).await;
                return Ok(text.to_string());
            }
            Message::Binary(bytes) => {
                let _ = socket.close(None).await;
                return Ok(String::from_utf8_lossy(&bytes).into_owned());
            }
            Message::Close(_) => break,
            Message::Ping(data) => {
                let _ = socket.send(Message::Pong(data)).await;
            }
            _ => {}
        }
    }
    Err(HarnessError::transport("websocket closed without a text frame"))
}
