//! Headless-browser (Playwright/Chromium) SPA discovery.
//!
//! Launches a real Chromium instance through a Node subprocess, navigates the
//! target, waits for single-page-app rendering, captures all network traffic
//! (with emphasis on XHR/fetch API calls), and exports the observed requests as
//! [`DiscoveredEndpoint`]s for the Discovery Engine.
//!
//! This is a real browser driver — no mock/stub driver exists. When Node or the
//! `playwright` package is not installed, [`BrowserCrawler::capture`] returns an
//! error so callers can degrade gracefully to HTTP-only discovery.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use promptlab_core::{PromptLabError, PromptLabResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tracing::{debug, info, instrument, warn};
use url::Url;

use crate::detectors::detect_from_snapshot;
use crate::types::{DiscoveredEndpoint, EndpointKind, HttpSnapshot};

/// SPA rendering wait strategy (maps to Playwright `waitUntil`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitUntil {
    /// Resolve after the `load` event.
    Load,
    /// Resolve after `DOMContentLoaded`.
    DomContentLoaded,
    /// Resolve after the network has been idle (best for SPAs).
    NetworkIdle,
    /// Resolve as soon as navigation is committed.
    Commit,
}

impl WaitUntil {
    fn as_playwright(self) -> &'static str {
        match self {
            Self::Load => "load",
            Self::DomContentLoaded => "domcontentloaded",
            Self::NetworkIdle => "networkidle",
            Self::Commit => "commit",
        }
    }
}

/// Configuration for the Playwright browser crawler.
#[derive(Debug, Clone)]
pub struct BrowserConfig {
    /// Node binary used to launch the runner (e.g. `node`).
    pub node_bin: String,
    /// Optional explicit path to `runner.mjs` (defaults to the bundled runner).
    pub runner_path: Option<PathBuf>,
    /// Run Chromium headless.
    pub headless: bool,
    /// Optional User-Agent override for the browser context.
    pub user_agent: Option<String>,
    /// Navigation wait strategy.
    pub wait_until: WaitUntil,
    /// Navigation timeout in milliseconds.
    pub nav_timeout_ms: u64,
    /// Extra settle time after navigation for late XHR/fetch calls.
    pub settle_ms: u64,
    /// Network-idle wait timeout after settling.
    pub idle_timeout_ms: u64,
    /// Maximum number of network requests to capture.
    pub max_requests: usize,
    /// Playwright storageState file for authenticated browser discovery.
    pub storage_state_path: Option<PathBuf>,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            node_bin: "node".into(),
            runner_path: None,
            headless: true,
            user_agent: Some(format!("PromptLab-Discovery/{}", env!("CARGO_PKG_VERSION"))),
            wait_until: WaitUntil::NetworkIdle,
            nav_timeout_ms: 30_000,
            settle_ms: 1_500,
            idle_timeout_ms: 5_000,
            max_requests: 1_000,
            storage_state_path: None,
        }
    }
}

/// A single network request observed by the browser during navigation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedRequest {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub resource_type: String,
    #[serde(default)]
    pub status: Option<u16>,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub from_main_frame: bool,
}

impl CapturedRequest {
    /// True when the request is an application API call (XHR / fetch).
    pub fn is_api(&self) -> bool {
        matches!(self.resource_type.as_str(), "xhr" | "fetch")
    }
}

/// Result of a single browser navigation + capture pass.
#[derive(Debug, Clone)]
pub struct BrowserCapture {
    /// URL after redirects / SPA routing.
    pub final_url: String,
    /// Document title after rendering.
    pub title: String,
    /// All captured network requests.
    pub requests: Vec<CapturedRequest>,
    /// Requests classified and exported as discovery endpoints.
    pub endpoints: Vec<DiscoveredEndpoint>,
    /// Non-fatal warnings (e.g. navigation timeout).
    pub errors: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct NavigateCaptureResult {
    #[serde(default)]
    final_url: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    requests: Vec<CapturedRequest>,
    #[serde(default)]
    nav_error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RunnerResponse {
    id: u64,
    ok: bool,
    #[serde(default)]
    result: Value,
    #[serde(default)]
    error: Option<String>,
}

struct RunnerProcess {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    next_id: AtomicU64,
}

/// Playwright-backed browser crawler for SPA discovery.
pub struct BrowserCrawler {
    config: BrowserConfig,
    proc: Arc<Mutex<RunnerProcess>>,
}

impl BrowserCrawler {
    pub fn new(config: BrowserConfig) -> Self {
        Self {
            config,
            proc: Arc::new(Mutex::new(RunnerProcess {
                child: None,
                stdin: None,
                stdout: None,
                next_id: AtomicU64::new(1),
            })),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(BrowserConfig::default())
    }

    pub fn config(&self) -> &BrowserConfig {
        &self.config
    }

    fn runner_path(&self) -> PromptLabResult<PathBuf> {
        if let Some(path) = &self.config.runner_path {
            return Ok(path.clone());
        }
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("playwright/runner.mjs");
        if path.exists() {
            Ok(path)
        } else {
            Err(PromptLabError::config(format!(
                "playwright runner not found at {}",
                path.display()
            )))
        }
    }

    /// Launch Chromium, navigate the target, wait for SPA rendering, and capture
    /// network traffic. Captured API requests are exported as endpoints.
    #[instrument(skip(self), fields(url = %url))]
    pub async fn capture(&self, url: &str) -> PromptLabResult<BrowserCapture> {
        let parsed =
            Url::parse(url).map_err(|e| PromptLabError::invalid_input(format!("invalid URL: {e}")))?;
        match parsed.scheme() {
            "http" | "https" => {}
            other => {
                return Err(PromptLabError::invalid_input(format!(
                    "unsupported scheme '{other}'; only http/https allowed"
                )))
            }
        }

        info!(%url, "launching browser capture");

        let payload = serde_json::json!({
            "url": url,
            "options": {
                "headless": self.config.headless,
                "user_agent": self.config.user_agent,
                "wait_until": self.config.wait_until.as_playwright(),
                "timeout_ms": self.config.nav_timeout_ms,
                "settle_ms": self.config.settle_ms,
                "idle_timeout_ms": self.config.idle_timeout_ms,
                "max_requests": self.config.max_requests,
                "storage_state_path": self.config.storage_state_path,
            }
        });

        let result: NavigateCaptureResult = self.call("navigate_capture", payload).await?;

        let source = if result.final_url.is_empty() {
            url.to_string()
        } else {
            result.final_url.clone()
        };

        let endpoints = endpoints_from_requests(&result.requests, &source);
        let mut errors = Vec::new();
        if let Some(err) = &result.nav_error {
            warn!(error = %err, "browser navigation warning");
            errors.push(format!("navigation warning: {err}"));
        }

        info!(
            requests = result.requests.len(),
            endpoints = endpoints.len(),
            "browser capture complete"
        );

        Ok(BrowserCapture {
            final_url: source,
            title: result.title,
            requests: result.requests,
            endpoints,
            errors,
        })
    }

    /// Gracefully close the browser and terminate the runner process.
    pub async fn close(&self) -> PromptLabResult<()> {
        let _ = self.call::<Value>("close", serde_json::json!({})).await;
        let mut guard = self.proc.lock().await;
        if let Some(mut child) = guard.child.take() {
            let _ = child.kill().await;
        }
        guard.stdin = None;
        guard.stdout = None;
        Ok(())
    }

    async fn ensure_process(&self) -> PromptLabResult<()> {
        let mut guard = self.proc.lock().await;
        if guard.child.is_some() {
            return Ok(());
        }

        let runner = self.runner_path()?;
        debug!(runner = %runner.display(), node = %self.config.node_bin, "spawning playwright runner");

        let mut child = Command::new(&self.config.node_bin)
            .arg(&runner)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                PromptLabError::internal(format!(
                    "failed to spawn playwright runner via '{}': {e}",
                    self.config.node_bin
                ))
            })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| PromptLabError::internal("runner stdin unavailable"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| PromptLabError::internal("runner stdout unavailable"))?;

        guard.child = Some(child);
        guard.stdin = Some(stdin);
        guard.stdout = Some(BufReader::new(stdout));
        Ok(())
    }

    async fn call<T: serde::de::DeserializeOwned>(
        &self,
        cmd: &str,
        payload: Value,
    ) -> PromptLabResult<T> {
        self.ensure_process().await?;

        let mut guard = self.proc.lock().await;
        let id = guard.next_id.fetch_add(1, Ordering::Relaxed);

        let mut body = serde_json::Map::new();
        body.insert("id".into(), id.into());
        body.insert("cmd".into(), cmd.into());
        if let Some(obj) = payload.as_object() {
            for (k, v) in obj {
                body.insert(k.clone(), v.clone());
            }
        }
        let line =
            serde_json::to_string(&body).map_err(|e| PromptLabError::internal(e.to_string()))?;

        {
            let stdin = guard
                .stdin
                .as_mut()
                .ok_or_else(|| PromptLabError::internal("runner stdin closed"))?;
            stdin
                .write_all(line.as_bytes())
                .await
                .map_err(|e| PromptLabError::internal(e.to_string()))?;
            stdin
                .write_all(b"\n")
                .await
                .map_err(|e| PromptLabError::internal(e.to_string()))?;
            stdin
                .flush()
                .await
                .map_err(|e| PromptLabError::internal(e.to_string()))?;
        }

        let mut response_line = String::new();
        {
            let stdout = guard
                .stdout
                .as_mut()
                .ok_or_else(|| PromptLabError::internal("runner stdout closed"))?;
            let n = stdout
                .read_line(&mut response_line)
                .await
                .map_err(|e| PromptLabError::internal(e.to_string()))?;
            if n == 0 {
                return Err(PromptLabError::internal(
                    "playwright runner closed unexpectedly (is the 'playwright' npm package installed?)",
                ));
            }
        }

        let response: RunnerResponse = serde_json::from_str(response_line.trim())
            .map_err(|e| PromptLabError::internal(format!("invalid runner response: {e}")))?;
        if response.id != id {
            return Err(PromptLabError::internal("runner response id mismatch"));
        }
        if !response.ok {
            return Err(PromptLabError::internal(
                response
                    .error
                    .unwrap_or_else(|| "playwright command failed".into()),
            ));
        }

        serde_json::from_value(response.result)
            .map_err(|e| PromptLabError::internal(format!("failed to decode runner result: {e}")))
    }
}

/// Convert captured network requests into deduplicated discovery endpoints.
pub fn endpoints_from_requests(
    requests: &[CapturedRequest],
    source_url: &str,
) -> Vec<DiscoveredEndpoint> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for req in requests {
        if let Some(ep) = classify_request(req, source_url) {
            let key = (ep.url.clone(), ep.kind, ep.method.clone());
            if seen.insert(key) {
                out.push(ep);
            }
        }
    }
    out
}

/// Classify a single captured request into a discovery endpoint, if relevant.
///
/// URL/status patterns are run through the shared detectors first; otherwise the
/// browser resource type is used (`xhr`/`fetch` → API, `script` → JavaScript).
/// Non-API resources (documents, images, stylesheets, fonts) are ignored.
fn classify_request(req: &CapturedRequest, source_url: &str) -> Option<DiscoveredEndpoint> {
    let snapshot = HttpSnapshot {
        url: req.url.clone(),
        status: req.status.unwrap_or(0),
        content_type: req.content_type.clone(),
        body: String::new(),
    };

    let detected = detect_from_snapshot(&snapshot, Some(source_url));

    let (kind, confidence) = match detected
        .into_iter()
        .max_by(|a, b| a.confidence.total_cmp(&b.confidence))
    {
        Some(best) => (best.kind, best.confidence),
        None => match req.resource_type.as_str() {
            "xhr" | "fetch" => (EndpointKind::RestApi, 0.7),
            "script" => (EndpointKind::JavaScript, 0.6),
            _ => return None,
        },
    };

    let evidence = match req.status {
        Some(status) => format!(
            "observed via browser network capture ({}, HTTP {status})",
            req.resource_type
        ),
        None => format!("observed via browser network capture ({})", req.resource_type),
    };

    Some(
        DiscoveredEndpoint::new(req.url.clone(), kind, confidence, evidence)
            .with_method(req.method.clone())
            .with_source(source_url),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(method: &str, url: &str, rt: &str, status: Option<u16>, ct: Option<&str>) -> CapturedRequest {
        CapturedRequest {
            method: method.into(),
            url: url.into(),
            resource_type: rt.into(),
            status,
            content_type: ct.map(Into::into),
            from_main_frame: true,
        }
    }

    #[test]
    fn wait_until_maps_to_playwright() {
        assert_eq!(WaitUntil::NetworkIdle.as_playwright(), "networkidle");
        assert_eq!(WaitUntil::DomContentLoaded.as_playwright(), "domcontentloaded");
        assert_eq!(WaitUntil::Load.as_playwright(), "load");
        assert_eq!(WaitUntil::Commit.as_playwright(), "commit");
    }

    #[test]
    fn classifies_xhr_api_request() {
        let r = req("GET", "https://app.example.com/api/v1/users", "xhr", Some(200), Some("application/json"));
        let ep = classify_request(&r, "https://app.example.com/").expect("endpoint");
        assert_eq!(ep.kind, EndpointKind::RestApi);
        assert_eq!(ep.method.as_deref(), Some("GET"));
        assert_eq!(ep.source_url.as_deref(), Some("https://app.example.com/"));
    }

    #[test]
    fn classifies_ai_endpoint_from_fetch() {
        let r = req("POST", "https://app.example.com/v1/chat/completions", "fetch", Some(401), Some("application/json"));
        let ep = classify_request(&r, "https://app.example.com/").expect("endpoint");
        assert_eq!(ep.kind, EndpointKind::AiEndpoint);
        assert_eq!(ep.method.as_deref(), Some("POST"));
    }

    #[test]
    fn classifies_generic_xhr_without_pattern_as_rest() {
        let r = req("POST", "https://app.example.com/data/load", "xhr", Some(200), Some("application/json"));
        let ep = classify_request(&r, "https://app.example.com/").expect("endpoint");
        assert_eq!(ep.kind, EndpointKind::RestApi);
    }

    #[test]
    fn classifies_script_as_javascript() {
        let r = req("GET", "https://app.example.com/static/bundle.js", "script", Some(200), Some("application/javascript"));
        let ep = classify_request(&r, "https://app.example.com/").expect("endpoint");
        assert_eq!(ep.kind, EndpointKind::JavaScript);
    }

    #[test]
    fn ignores_non_api_resources() {
        assert!(classify_request(&req("GET", "https://x/logo.png", "image", Some(200), Some("image/png")), "https://x/").is_none());
        assert!(classify_request(&req("GET", "https://x/", "document", Some(200), Some("text/html")), "https://x/").is_none());
        assert!(classify_request(&req("GET", "https://x/app.css", "stylesheet", Some(200), Some("text/css")), "https://x/").is_none());
    }

    #[test]
    fn deduplicates_repeated_requests() {
        let reqs = vec![
            req("GET", "https://x/api/items", "xhr", Some(200), Some("application/json")),
            req("GET", "https://x/api/items", "xhr", Some(200), Some("application/json")),
            req("POST", "https://x/api/items", "xhr", Some(201), Some("application/json")),
        ];
        let eps = endpoints_from_requests(&reqs, "https://x/");
        // Two distinct (url, kind, method) keys: GET and POST.
        assert_eq!(eps.len(), 2);
    }

    #[test]
    fn parses_runner_navigate_result() {
        let json = r#"{
            "final_url": "https://app.example.com/dashboard",
            "title": "Dashboard",
            "requests": [
                {"method":"GET","url":"https://app.example.com/api/v1/me","resource_type":"xhr","status":200,"content_type":"application/json","from_main_frame":true},
                {"method":"GET","url":"https://app.example.com/static/app.js","resource_type":"script","status":200,"content_type":"text/javascript","from_main_frame":true}
            ],
            "nav_error": null
        }"#;
        let parsed: NavigateCaptureResult = serde_json::from_str(json).expect("parse");
        assert_eq!(parsed.requests.len(), 2);
        let eps = endpoints_from_requests(&parsed.requests, &parsed.final_url);
        assert!(eps.iter().any(|e| e.kind == EndpointKind::RestApi && e.url.contains("/api/v1/me")));
        assert!(eps.iter().any(|e| e.kind == EndpointKind::JavaScript));
    }
}
