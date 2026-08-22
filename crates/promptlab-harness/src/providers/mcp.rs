use async_trait::async_trait;
use serde_json::{json, Value};

use crate::error::{HarnessError, HarnessResult};
use crate::models::{AttackRequest, HttpMethod, NormalizedResponse};
use crate::providers::HttpHarness;
use crate::traits::Harness;

/// MCP JSON-RPC over Streamable HTTP.
#[derive(Clone)]
pub struct McpHarness {
    inner: HttpHarness,
}

impl McpHarness {
    pub fn new() -> HarnessResult<Self> {
        Ok(Self {
            inner: HttpHarness::new()?,
        })
    }

    async fn rpc(
        &self,
        base: &AttackRequest,
        session_id: Option<&str>,
        payload: Value,
    ) -> HarnessResult<NormalizedResponse> {
        let mut request = base.clone();
        request.method = HttpMethod::Post;
        request.body = Some(payload.to_string());
        request.payload = String::new();
        if let Some(session) = session_id.filter(|s| !s.is_empty()) {
            request
                .headers
                .insert("mcp-session-id".into(), session.to_string());
        }
        if !request
            .headers
            .keys()
            .any(|k| k.eq_ignore_ascii_case("accept"))
        {
            request
                .headers
                .insert("Accept".into(), "application/json, text/event-stream".into());
        }
        self.inner.execute_raw(&request, self.id()).await
    }
}

impl Default for McpHarness {
    fn default() -> Self {
        Self::new().expect("mcp harness")
    }
}

#[async_trait]
impl Harness for McpHarness {
    fn id(&self) -> &'static str {
        "mcp"
    }

    async fn execute(&self, request: AttackRequest) -> HarnessResult<NormalizedResponse> {
        if request.url.starts_with("stdio:") || request.url.starts_with("stdio://") {
            return execute_stdio(&request).await;
        }
        let body = request.effective_body();
        if looks_like_jsonrpc(&body) {
            let mut req = request.clone();
            req.body = Some(body);
            let mut response = self.inner.execute_raw(&req, self.id()).await?;
            response
                .metadata
                .insert("api_format".into(), "mcp_jsonrpc".into());
            return Ok(response);
        }

        let init = self
            .rpc(
                &request,
                request.mcp_session_id.as_deref(),
                json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {},
                        "clientInfo": { "name": "promptlab", "version": "0.1.0" }
                    }
                }),
            )
            .await?;
        if init.status_code.unwrap_or(0) >= 400 {
            let mut init = init;
            init.metadata
                .insert("api_format".into(), "mcp_initialize".into());
            return Ok(init);
        }
        let session = init
            .metadata
            .get("mcp_session_id")
            .cloned()
            .or_else(|| request.mcp_session_id.clone());

        let listed = self
            .rpc(
                &request,
                session.as_deref(),
                json!({
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "tools/list",
                    "params": {}
                }),
            )
            .await?;

        let method = request
            .mcp_method
            .clone()
            .unwrap_or_else(|| "tools/call".into());
        if method == "tools/list" {
            let mut listed = listed;
            listed.metadata.insert("api_format".into(), "mcp_jsonrpc".into());
            if let Some(session) = session {
                listed.metadata.insert("mcp_session_id".into(), session);
            }
            return Ok(listed);
        }

        let tool_name = first_tool_name(&listed).unwrap_or_else(|| "run".into());
        let mut response = self
            .rpc(
                &request,
                session.as_deref(),
                json!({
                    "jsonrpc": "2.0",
                    "id": 3,
                    "method": method,
                    "params": {
                        "name": tool_name,
                        "arguments": { "input": request.payload }
                    }
                }),
            )
            .await?;
        response
            .metadata
            .insert("api_format".into(), "mcp_jsonrpc".into());
        if let Some(session) = session {
            response.metadata.insert("mcp_session_id".into(), session);
        }
        Ok(response)
    }
}

fn looks_like_jsonrpc(body: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return false;
    };
    value.get("jsonrpc").and_then(|v| v.as_str()) == Some("2.0") && value.get("method").is_some()
}

fn first_tool_name(list_response: &NormalizedResponse) -> Option<String> {
    let json: Value = serde_json::from_str(&list_response.raw_response).ok()?;
    json.pointer("/result/tools/0/name")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

async fn execute_stdio(request: &AttackRequest) -> HarnessResult<NormalizedResponse> {
    request.cancel.check()?;
    let cmdline = request
        .file_path
        .clone()
        .unwrap_or_else(|| {
            request
                .url
                .trim_start_matches("stdio://")
                .trim_start_matches("stdio:")
                .trim()
                .to_string()
        });
    if cmdline.is_empty() {
        return Err(HarnessError::config(
            "mcp stdio requires stdio:<command> or request.file_path",
        ));
    }
    let mut parts = cmdline.split_whitespace();
    let program = parts.next().unwrap_or("").to_string();
    let args: Vec<String> = parts.map(str::to_string).collect();
    let mut child = tokio::process::Command::new(&program)
        .args(&args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|err| HarnessError::transport(format!("mcp stdio spawn {program}: {err}")))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| HarnessError::transport("mcp stdio missing stdin"))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| HarnessError::transport("mcp stdio missing stdout"))?;

    let timeout = std::time::Duration::from_millis(request.timeout_ms.max(1));
    let outcome = tokio::time::timeout(timeout, async {
        let init = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "promptlab", "version": "0.1.0" }
            }
        });
        write_rpc(&mut stdin, &init).await?;
        let _initialized = read_rpc(&mut stdout, request).await?;
        write_rpc(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            }),
        )
        .await?;
        let method = request
            .mcp_method
            .clone()
            .unwrap_or_else(|| "tools/call".into());
        if method == "tools/list" {
            write_rpc(
                &mut stdin,
                &json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
            )
            .await?;
            return read_rpc(&mut stdout, request).await;
        }
        write_rpc(
            &mut stdin,
            &json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        )
        .await?;
        let listed = read_rpc(&mut stdout, request).await?;
        let tool_name = serde_json::from_str::<Value>(&listed)
            .ok()
            .and_then(|value| {
                value
                    .pointer("/result/tools/0/name")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "run".into());
        write_rpc(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": method,
                "params": {
                    "name": tool_name,
                    "arguments": { "input": request.payload }
                }
            }),
        )
        .await?;
        read_rpc(&mut stdout, request).await
    })
    .await
    .map_err(|_| HarnessError::Timeout("mcp stdio timed out".into()))?;

    let _ = child.kill().await;
    let raw = outcome?;
    let mut response = NormalizedResponse::from_http(200, raw, "mcp");
    response
        .metadata
        .insert("api_format".into(), "mcp_stdio".into());
    response
        .metadata
        .insert("transport".into(), "stdio".into());
    Ok(response)
}

async fn write_rpc<W: tokio::io::AsyncWriteExt + Unpin>(
    writer: &mut W,
    payload: &Value,
) -> HarnessResult<()> {
    let body = serde_json::to_vec(payload)?;
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    writer
        .write_all(header.as_bytes())
        .await
        .map_err(|err| HarnessError::transport(err.to_string()))?;
    writer
        .write_all(&body)
        .await
        .map_err(|err| HarnessError::transport(err.to_string()))?;
    writer
        .flush()
        .await
        .map_err(|err| HarnessError::transport(err.to_string()))
}

async fn read_rpc<R: tokio::io::AsyncReadExt + Unpin>(
    reader: &mut R,
    request: &AttackRequest,
) -> HarnessResult<String> {
    let mut header = Vec::new();
    loop {
        request.cancel.check()?;
        let mut byte = [0u8; 1];
        reader
            .read_exact(&mut byte)
            .await
            .map_err(|err| HarnessError::transport(format!("mcp stdio header: {err}")))?;
        header.push(byte[0]);
        if header.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if header.len() > 8_192 {
            return Err(HarnessError::transport("mcp stdio header too large"));
        }
    }
    let header_text = String::from_utf8_lossy(&header);
    let length = header_text
        .lines()
        .find_map(|line| {
            line.split_once(':').and_then(|(name, value)| {
                if name.eq_ignore_ascii_case("content-length") {
                    value.trim().parse::<usize>().ok()
                } else {
                    None
                }
            })
        })
        .ok_or_else(|| HarnessError::transport("mcp stdio missing Content-Length"))?;
    if length > 2 * 1024 * 1024 {
        return Err(HarnessError::transport("mcp stdio body exceeds 2MiB"));
    }
    let mut body = vec![0u8; length];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|err| HarnessError::transport(format!("mcp stdio body: {err}")))?;
    String::from_utf8(body).map_err(|err| HarnessError::transport(err.to_string()))
}
