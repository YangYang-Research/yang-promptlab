use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use aisec_core::{AisecError, AisecResult};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;
use tracing::instrument;

use crate::config::AuthEngineConfig;
use crate::playwright::protocol::{
    PlaywrightOptions, PlaywrightResponse, RecordLoginRequest, RecordLoginResult,
    ReplaySessionRequest, ReplaySessionResult,
};
use crate::types::{CookieRecord, ExtractedToken, RecordLoginOptions, ReplayOptions};

/// Playwright automation driver (subprocess JSON-lines protocol).
#[async_trait]
pub trait PlaywrightDriver: Send + Sync {
    async fn launch(&self, options: PlaywrightOptions) -> AisecResult<()>;
    async fn close(&self) -> AisecResult<()>;
    async fn record_login(
        &self,
        url: &str,
        method: &str,
        config: Value,
        options: RecordLoginOptions,
    ) -> AisecResult<RecordLoginResult>;
    async fn begin_interactive_login(
        &self,
        url: &str,
        options: RecordLoginOptions,
    ) -> AisecResult<()>;
    async fn finish_interactive_login(&self) -> AisecResult<RecordLoginResult>;
    async fn replay_session(
        &self,
        url: &str,
        storage_state: Option<Value>,
        storage_state_path: Option<&Path>,
        options: ReplayOptions,
    ) -> AisecResult<ReplaySessionResult>;
    async fn extract_tokens(&self, url: Option<&str>) -> AisecResult<Vec<ExtractedToken>>;
    async fn get_cookies(&self, url: Option<&str>) -> AisecResult<Vec<CookieRecord>>;
    async fn set_cookies(&self, cookies: Vec<CookieRecord>) -> AisecResult<Vec<CookieRecord>>;
}

pub struct PlaywrightClient {
    inner: Arc<Mutex<PlaywrightProcess>>,
    config: AuthEngineConfig,
}

struct PlaywrightProcess {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<tokio::process::ChildStdout>>,
    next_id: AtomicU64,
}

impl PlaywrightClient {
    pub async fn new(config: AuthEngineConfig) -> AisecResult<Self> {
        Ok(Self {
            inner: Arc::new(Mutex::new(PlaywrightProcess {
                child: None,
                stdin: None,
                stdout: None,
                next_id: AtomicU64::new(1),
            })),
            config,
        })
    }

    fn runner_path(&self) -> AisecResult<PathBuf> {
        if let Some(path) = &self.config.playwright_runner {
            return Ok(path.clone());
        }
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("playwright/runner.mjs");
        if manifest.exists() {
            Ok(manifest)
        } else {
            Err(AisecError::config(format!(
                "playwright runner not found at {}",
                manifest.display()
            )))
        }
    }

    async fn ensure_process(&self) -> AisecResult<()> {
        let mut guard = self.inner.lock().await;
        if guard.child.is_some() {
            return Ok(());
        }

        let runner = self.runner_path()?;
        let mut command = Command::new(&self.config.node_bin);
        command
            .arg(&runner)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true);

        if let Some(workdir) = &self.config.runner_workdir {
            command.current_dir(workdir);
        }
        if let Some(browsers_path) = &self.config.playwright_browsers_path {
            command.env("PLAYWRIGHT_BROWSERS_PATH", browsers_path);
        }

        let mut child = command
            .spawn()
            .map_err(|err| AisecError::internal(format!("failed to spawn playwright runner: {err}")))?;

        let stdin = child.stdin.take().ok_or_else(|| AisecError::internal("no stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| AisecError::internal("no stdout"))?;

        guard.child = Some(child);
        guard.stdin = Some(stdin);
        guard.stdout = Some(BufReader::new(stdout));
        Ok(())
    }

    async fn call<T: DeserializeOwned>(&self, cmd: &str, payload: Value) -> AisecResult<T> {
        self.ensure_process().await?;

        let id = {
            let guard = self.inner.lock().await;
            guard.next_id.fetch_add(1, Ordering::Relaxed)
        };

        let mut body = serde_json::Map::new();
        body.insert("id".into(), id.into());
        body.insert("cmd".into(), cmd.into());
        if let Some(obj) = payload.as_object() {
            for (k, v) in obj {
                body.insert(k.clone(), v.clone());
            }
        }

        let line =
            serde_json::to_string(&body).map_err(|e| AisecError::internal(e.to_string()))?;

        {
            let mut guard = self.inner.lock().await;
            let stdin = guard
                .stdin
                .as_mut()
                .ok_or_else(|| AisecError::internal("stdin closed"))?;
            stdin
                .write_all(line.as_bytes())
                .await
                .map_err(|e| AisecError::internal(e.to_string()))?;
            stdin
                .write_all(b"\n")
                .await
                .map_err(|e| AisecError::internal(e.to_string()))?;
            stdin
                .flush()
                .await
                .map_err(|e| AisecError::internal(e.to_string()))?;
        }

        let response_line = {
            let mut guard = self.inner.lock().await;
            let stdout = guard
                .stdout
                .as_mut()
                .ok_or_else(|| AisecError::internal("stdout closed"))?;
            let mut buf = String::new();
            stdout
                .read_line(&mut buf)
                .await
                .map_err(|e| AisecError::internal(e.to_string()))?;
            buf
        };

        let response: PlaywrightResponse = serde_json::from_str(&response_line)
            .map_err(|e| AisecError::internal(format!("invalid playwright response: {e}")))?;

        if response.id != id {
            return Err(AisecError::internal("playwright response id mismatch"));
        }
        if !response.ok {
            return Err(AisecError::internal(
                response
                    .error
                    .unwrap_or_else(|| "playwright command failed".into()),
            ));
        }

        serde_json::from_value(response.result)
            .map_err(|e| AisecError::internal(format!("failed to decode playwright result: {e}")))
    }
}

#[async_trait]
impl PlaywrightDriver for PlaywrightClient {
    #[instrument(skip(self))]
    async fn launch(&self, options: PlaywrightOptions) -> AisecResult<()> {
        let _: serde_json::Value = self
            .call("launch", serde_json::json!({ "options": options }))
            .await?;
        Ok(())
    }

    async fn close(&self) -> AisecResult<()> {
        let _: serde_json::Value = self.call("close", serde_json::json!({})).await?;
        let mut guard = self.inner.lock().await;
        if let Some(mut child) = guard.child.take() {
            let _ = child.kill().await;
        }
        guard.stdin = None;
        guard.stdout = None;
        Ok(())
    }

    async fn record_login(
        &self,
        url: &str,
        method: &str,
        config: Value,
        options: RecordLoginOptions,
    ) -> AisecResult<RecordLoginResult> {
        self.launch(PlaywrightOptions {
            headless: !options.headed,
            headed: options.headed,
            timeout_ms: options.timeout_ms,
            interactive_timeout_ms: options.interactive_timeout_ms,
            ..Default::default()
        })
        .await?;

        let req = RecordLoginRequest {
            url: url.to_string(),
            method: method.to_string(),
            config,
            options: PlaywrightOptions {
                headless: !options.headed,
                headed: options.headed,
                timeout_ms: options.timeout_ms,
                interactive_timeout_ms: options.interactive_timeout_ms,
                ..Default::default()
            },
        };

        self.call("record_login", serde_json::to_value(req).unwrap())
            .await
    }

    async fn begin_interactive_login(
        &self,
        url: &str,
        options: RecordLoginOptions,
    ) -> AisecResult<()> {
        self.launch(PlaywrightOptions {
            headless: false,
            headed: true,
            timeout_ms: options.timeout_ms,
            interactive_timeout_ms: options.interactive_timeout_ms,
            ..Default::default()
        })
        .await?;

        let _: serde_json::Value = self
            .call(
                "begin_interactive_login",
                serde_json::json!({
                    "url": url,
                    "options": PlaywrightOptions {
                        headless: false,
                        headed: true,
                        timeout_ms: options.timeout_ms,
                        interactive_timeout_ms: options.interactive_timeout_ms,
                        ..Default::default()
                    }
                }),
            )
            .await?;
        Ok(())
    }

    async fn finish_interactive_login(&self) -> AisecResult<RecordLoginResult> {
        self.call("finish_interactive_login", serde_json::json!({}))
            .await
    }

    async fn replay_session(
        &self,
        url: &str,
        storage_state: Option<Value>,
        storage_state_path: Option<&Path>,
        options: ReplayOptions,
    ) -> AisecResult<ReplaySessionResult> {
        let req = ReplaySessionRequest {
            url: url.to_string(),
            storage_state,
            storage_state_path: storage_state_path.map(|p| p.to_string_lossy().into_owned()),
            options: PlaywrightOptions {
                headless: !options.headed,
                headed: options.headed,
                timeout_ms: options.timeout_ms,
                storage_state_path: storage_state_path.map(|p| p.to_string_lossy().into_owned()),
                ..Default::default()
            },
        };
        self.call("replay_session", serde_json::to_value(req).unwrap())
            .await
    }

    async fn extract_tokens(&self, url: Option<&str>) -> AisecResult<Vec<ExtractedToken>> {
        let result: serde_json::Value = self
            .call(
                "extract_tokens",
                serde_json::json!({ "url": url, "options": PlaywrightOptions::default() }),
            )
            .await?;
        parse_tokens(&result["tokens"])
    }

    async fn get_cookies(&self, url: Option<&str>) -> AisecResult<Vec<CookieRecord>> {
        let result: serde_json::Value = self
            .call("get_cookies", serde_json::json!({ "url": url }))
            .await?;
        parse_cookies(&result["cookies"])
    }

    async fn set_cookies(&self, cookies: Vec<CookieRecord>) -> AisecResult<Vec<CookieRecord>> {
        let result: serde_json::Value = self
            .call("set_cookies", serde_json::json!({ "cookies": cookies }))
            .await?;
        parse_cookies(&result["cookies"])
    }
}

pub fn parse_tokens(value: &Value) -> AisecResult<Vec<ExtractedToken>> {
    serde_json::from_value(value.clone())
        .map_err(|e| AisecError::internal(format!("token parse error: {e}")))
}

pub fn parse_cookies(value: &Value) -> AisecResult<Vec<CookieRecord>> {
    serde_json::from_value(value.clone())
        .map_err(|e| AisecError::internal(format!("cookie parse error: {e}")))
}
