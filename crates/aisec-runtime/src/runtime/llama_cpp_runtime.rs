//! Embedded llama.cpp server runtime (`llama-server` subprocess + HTTP API).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::error::{RuntimeError, RuntimeResult};
use crate::runtime::gguf::{validate_gguf_model, GgufQuantization};

#[derive(Debug, Clone)]
pub struct LlamaCppRuntimeConfig {
    pub binary_path: PathBuf,
    pub host: String,
    pub port: u16,
    pub n_gpu_layers: u32,
    pub ctx_size: u32,
    pub startup_timeout_ms: u64,
}

impl LlamaCppRuntimeConfig {
    pub fn from_binary(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary_path: binary.into(),
            host: default_llama_host(),
            port: default_llama_port(),
            n_gpu_layers: default_n_gpu_layers(),
            ctx_size: 4096,
            startup_timeout_ms: default_startup_timeout_ms(),
        }
    }

    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeState {
    Unloaded = 0,
    Loading = 1,
    Ready = 2,
    Error = 3,
}

/// Inference request for the embedded llama.cpp HTTP API.
#[derive(Debug, Clone, Serialize)]
pub struct InferRequest {
    pub prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

/// Inference response from `/completion`.
#[derive(Debug, Clone)]
pub struct InferResponse {
    pub text: String,
    pub tokens_predicted: u32,
    pub duration_ms: u64,
    pub quantization: Option<GgufQuantization>,
}

/// Manages a `llama-server` subprocess bound to a single GGUF model.
pub struct LlamaCppRuntime {
    config: LlamaCppRuntimeConfig,
    client: reqwest::Client,
    process: Mutex<Option<Child>>,
    model_path: Mutex<Option<PathBuf>>,
    quantization: Mutex<Option<GgufQuantization>>,
    state: AtomicU32,
}

impl LlamaCppRuntime {
    pub fn new(config: LlamaCppRuntimeConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
            process: Mutex::new(None),
            model_path: Mutex::new(None),
            quantization: Mutex::new(None),
            state: AtomicU32::new(RuntimeState::Unloaded as u32),
        }
    }

    pub fn config(&self) -> &LlamaCppRuntimeConfig {
        &self.config
    }

    pub fn base_url(&self) -> String {
        self.config.base_url()
    }

    pub fn binary_available(&self) -> bool {
        self.config.binary_path.is_file()
    }

    pub fn is_loaded(&self) -> bool {
        self.state() == RuntimeState::Ready
    }

    fn state(&self) -> RuntimeState {
        match self.state.load(Ordering::SeqCst) {
            x if x == RuntimeState::Unloaded as u32 => RuntimeState::Unloaded,
            x if x == RuntimeState::Loading as u32 => RuntimeState::Loading,
            x if x == RuntimeState::Ready as u32 => RuntimeState::Ready,
            _ => RuntimeState::Error,
        }
    }

    fn set_state(&self, state: RuntimeState) {
        self.state.store(state as u32, Ordering::SeqCst);
    }

    /// Load a GGUF model and start `llama-server`.
    pub async fn load_model(&self, model_path: &Path) -> RuntimeResult<()> {
        let quant = validate_gguf_model(model_path)?;
        self.shutdown().await.ok();
        self.set_state(RuntimeState::Loading);

        let mut child = Command::new(&self.config.binary_path)
            .current_dir(
                self.config
                    .binary_path
                    .parent()
                    .filter(|p| p.is_dir())
                    .unwrap_or_else(|| Path::new(".")),
            )
            .arg("-m")
            .arg(model_path)
            .arg("--host")
            .arg(&self.config.host)
            .arg("--port")
            .arg(self.config.port.to_string())
            .arg("-ngl")
            .arg(self.config.n_gpu_layers.to_string())
            .arg("-c")
            .arg(self.config.ctx_size.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|err| {
                self.set_state(RuntimeState::Error);
                RuntimeError::Process(format!(
                    "failed to spawn {}: {err}",
                    self.config.binary_path.display()
                ))
            })?;

        *self.process.lock().await = Some(child);

        let startup_timeout_ms = startup_timeout_for_model(model_path, self.config.startup_timeout_ms).await;
        info!(
            model = %model_path.display(),
            timeout_ms = startup_timeout_ms,
            "loading GGUF into llama-server"
        );

        let deadline = Instant::now() + Duration::from_millis(startup_timeout_ms);
        while Instant::now() < deadline {
            {
                let mut guard = self.process.lock().await;
                if let Some(proc) = guard.as_mut() {
                    if let Ok(Some(status)) = proc.try_wait() {
                        let stderr = proc.stderr.take();
                        drop(guard);
                        let mut detail = format!("llama-server exited during startup: {status}");
                        if let Some(mut stderr) = stderr {
                            use tokio::io::AsyncReadExt;
                            let mut buf = Vec::new();
                            if stderr.read_to_end(&mut buf).await.unwrap_or(0) > 0 {
                                let text = String::from_utf8_lossy(&buf);
                                if !text.trim().is_empty() {
                                    detail.push_str(" — ");
                                    detail.push_str(text.trim());
                                }
                            }
                        }
                        self.shutdown().await.ok();
                        self.set_state(RuntimeState::Error);
                        return Err(RuntimeError::Process(detail));
                    }
                }
            }
            if self.health().await? {
                *self.model_path.lock().await = Some(model_path.to_path_buf());
                *self.quantization.lock().await = Some(quant);
                self.set_state(RuntimeState::Ready);
                info!(
                    model = %model_path.display(),
                    quant = quant.as_str(),
                    base_url = %self.base_url(),
                    "llama.cpp runtime ready"
                );
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        self.shutdown().await.ok();
        self.set_state(RuntimeState::Error);
        Err(RuntimeError::Process(format!(
            "llama.cpp server startup timeout after {}s — large CPU models may need AISEC_LLAMA_STARTUP_TIMEOUT_MS",
            startup_timeout_ms / 1000
        )))
    }

    /// Unload the active model and stop the server process.
    pub async fn unload_model(&self) -> RuntimeResult<()> {
        self.shutdown().await
    }

    /// Run a completion against the loaded model.
    pub async fn infer(&self, request: InferRequest) -> RuntimeResult<InferResponse> {
        if self.state() != RuntimeState::Ready {
            return Err(RuntimeError::Process("runtime not ready".into()));
        }

        let started = Instant::now();
        let body = serde_json::json!({
            "prompt": request.prompt,
            "n_predict": request.max_tokens,
            "temperature": request.temperature,
            "stream": false,
        });

        let response = self
            .client
            .post(format!("{}/completion", self.base_url()))
            .json(&body)
            .send()
            .await
            .map_err(|err| RuntimeError::Process(err.to_string()))?;

        if !response.status().is_success() {
            return Err(RuntimeError::Process(format!(
                "completion HTTP {}",
                response.status()
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|err| RuntimeError::Process(err.to_string()))?;

        let text = json
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let tokens = json
            .get("tokens_predicted")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        debug!(
            tokens,
            duration_ms = started.elapsed().as_millis(),
            "llama.cpp inference complete"
        );

        Ok(InferResponse {
            text,
            tokens_predicted: tokens,
            duration_ms: started.elapsed().as_millis() as u64,
            quantization: *self.quantization.lock().await,
        })
    }

    /// Probe `/health` on the llama-server HTTP API.
    pub async fn health(&self) -> RuntimeResult<bool> {
        let url = format!("{}/health", self.base_url());
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .map_err(|err| RuntimeError::Process(err.to_string()))?;
        match client.get(&url).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(err) => {
                warn!(%err, "llama.cpp health check failed");
                Ok(false)
            }
        }
    }

    /// Returns false when the supervised subprocess has exited.
    pub async fn subprocess_running(&self) -> bool {
        let mut guard = self.process.lock().await;
        let Some(proc) = guard.as_mut() else {
            return false;
        };
        matches!(proc.try_wait(), Ok(None))
    }

    /// Stop the server subprocess and reset state.
    pub async fn shutdown(&self) -> RuntimeResult<()> {
        if let Some(mut child) = self.process.lock().await.take() {
            child.kill().await.ok();
            child.wait().await.ok();
        }
        *self.model_path.lock().await = None;
        *self.quantization.lock().await = None;
        self.set_state(RuntimeState::Unloaded);
        Ok(())
    }

    pub async fn loaded_model_path(&self) -> Option<PathBuf> {
        self.model_path.lock().await.clone()
    }

    pub async fn pid(&self) -> Option<u32> {
        self.process
            .lock()
            .await
            .as_ref()
            .and_then(|child| child.id())
    }
}

pub fn default_llama_port() -> u16 {
    std::env::var("AISEC_LLAMA_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8081)
}

pub fn default_llama_host() -> String {
    std::env::var("AISEC_LLAMA_HOST")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "127.0.0.1".into())
}

pub fn default_startup_timeout_ms() -> u64 {
    std::env::var("AISEC_LLAMA_STARTUP_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120_000)
}

pub fn default_n_gpu_layers() -> u32 {
    std::env::var("AISEC_LLAMA_N_GPU_LAYERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            if cfg!(target_os = "macos") {
                99
            } else {
                0
            }
        })
}

async fn startup_timeout_for_model(model_path: &Path, config_default_ms: u64) -> u64 {
    if let Ok(raw) = std::env::var("AISEC_LLAMA_STARTUP_TIMEOUT_MS") {
        if let Ok(ms) = raw.parse::<u64>() {
            return ms;
        }
    }

    let size_bytes = tokio::fs::metadata(model_path)
        .await
        .map(|meta| meta.len())
        .unwrap_or(0);
    let size_gb = size_bytes.saturating_div(1_000_000_000);
    // 90s base + 90s per GB for mmap/load on CPU; cap at 10 minutes.
    let computed = 90_000u64.saturating_add(size_gb.saturating_mul(90_000));
    computed.max(config_default_ms).min(600_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_base_url_uses_port() {
        let cfg = LlamaCppRuntimeConfig::from_binary("llama-server");
        assert!(cfg.base_url().contains(":8081") || cfg.port != 8081);
    }
}
