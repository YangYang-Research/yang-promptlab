use std::path::Path;
use std::process::Stdio;
use std::time::Instant;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use tracing::debug;

use crate::error::{PluginError, PluginResult};
use crate::manifest::PluginManifest;
use crate::permissions::PermissionGuard;
use crate::types::{HostCallRecord, PluginInvokeResult, SandboxConfig};

/// JSON-lines protocol message from host to plugin.
#[derive(Debug, serde::Serialize)]
struct InvokeRequest {
    id: String,
    method: String,
    params: serde_json::Value,
}

/// JSON-lines response from plugin.
#[derive(Debug, serde::Deserialize)]
struct PluginLine {
    id: Option<String>,
    result: Option<serde_json::Value>,
    error: Option<PluginLineError>,
    #[serde(rename = "type")]
    line_type: Option<String>,
    method: Option<String>,
    params: Option<serde_json::Value>,
}

#[derive(Debug, serde::Deserialize)]
struct PluginLineError {
    message: String,
}

/// Sandboxed subprocess runner for Python/JavaScript plugins.
pub struct SandboxRunner {
    config: SandboxConfig,
}

impl SandboxRunner {
    pub fn new(config: SandboxConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(SandboxConfig::default())
    }

    pub async fn invoke(
        &self,
        manifest: &PluginManifest,
        plugin_dir: &Path,
        hook: &str,
        params: serde_json::Value,
        guard: &PermissionGuard,
    ) -> PluginResult<PluginInvokeResult> {
        let started = Instant::now();
        let interpreter = manifest.interpreter()?;
        let entry = manifest.entry_path(plugin_dir);

        if !entry.exists() {
            return Err(PluginError::sandbox(format!(
                "entry not found: {}",
                entry.display()
            )));
        }

        let mut cmd = Command::new(&interpreter);
        cmd.arg(&entry)
            .current_dir(plugin_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("PROMPTLAB_PLUGIN_ID", &manifest.plugin.id)
            .env("PROMPTLAB_PLUGIN_DIR", plugin_dir)
            .env("PROMPTLAB_HOST_API", crate::manifest::HOST_API_VERSION)
            .env("PROMPTLAB_SANDBOX", "1");

        if !self.config.allow_network_env {
            cmd.env("PROMPTLAB_NO_NETWORK", "1");
        }

        // Strip inherited secrets from environment
        cmd.env_remove("AWS_SECRET_ACCESS_KEY");
        cmd.env_remove("OPENAI_API_KEY");

        let mut child = cmd
            .spawn()
            .map_err(|e| PluginError::sandbox(format!("spawn failed: {e}")))?;

        let stdin = child.stdin.take().ok_or_else(|| PluginError::sandbox("no stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| PluginError::sandbox("no stdout"))?;

        let request_id = uuid::Uuid::new_v4().to_string();
        let request = InvokeRequest {
            id: request_id.clone(),
            method: hook.to_string(),
            params,
        };
        let request_line = serde_json::to_string(&request)
            .map_err(|e| PluginError::sandbox(e.to_string()))?;

        let mut stdin = stdin;
        stdin
            .write_all(format!("{request_line}\n").as_bytes())
            .await
            .map_err(|e| PluginError::sandbox(e.to_string()))?;
        stdin
            .write_all(b"{\"type\":\"shutdown\"}\n")
            .await
            .map_err(|e| PluginError::sandbox(e.to_string()))?;
        drop(stdin);

        let duration = Duration::from_millis(self.config.timeout_ms);
        let read_future = read_plugin_output(stdout, &request_id, guard);
        let (result, host_calls) = timeout(duration, read_future)
            .await
            .map_err(|_| PluginError::sandbox("plugin execution timeout"))?
            .map_err(|e| PluginError::sandbox(e.to_string()))?;

        let _ = child.kill().await;

        debug!(
            plugin_id = %manifest.plugin.id,
            hook,
            duration_ms = started.elapsed().as_millis(),
            "plugin invoke complete"
        );

        Ok(PluginInvokeResult {
            plugin_id: manifest.plugin.id.clone(),
            hook: hook.to_string(),
            result,
            host_calls,
            duration_ms: started.elapsed().as_millis() as u64,
        })
    }
}

async fn read_plugin_output(
    stdout: tokio::process::ChildStdout,
    request_id: &str,
    guard: &PermissionGuard,
) -> Result<(serde_json::Value, Vec<HostCallRecord>), String> {
    let mut reader = BufReader::new(stdout).lines();
    let mut host_calls = Vec::new();
    let mut final_result = serde_json::json!({});

    while let Some(line) = reader
        .next_line()
        .await
        .map_err(|e| e.to_string())?
    {
        if line.trim().is_empty() {
            continue;
        }
        let parsed: PluginLine =
            serde_json::from_str(&line).map_err(|e| format!("invalid plugin output: {e}"))?;

        if parsed.line_type.as_deref() == Some("host") {
            let method = parsed.method.unwrap_or_default();
            let params = parsed.params.unwrap_or(serde_json::json!({}));
            let allowed = guard.check(&method).is_ok();
            host_calls.push(HostCallRecord {
                method,
                allowed,
                params,
            });
            continue;
        }

        if parsed.id.as_deref() == Some(request_id) {
            if let Some(err) = parsed.error {
                return Err(err.message);
            }
            if let Some(result) = parsed.result {
                final_result = result;
            }
        }
    }

    Ok((final_result, host_calls))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PluginPermissions;

    #[test]
    fn invoke_request_serializes() {
        let req = InvokeRequest {
            id: "1".into(),
            method: "discover".into(),
            params: serde_json::json!({"url": "https://example.com"}),
        };
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains("discover"));
    }
}
