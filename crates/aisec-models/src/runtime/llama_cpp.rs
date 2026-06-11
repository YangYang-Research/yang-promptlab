use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::error::{ModelError, ModelResult};
use crate::runtime::InferenceRuntime;
use crate::types::{InferenceRequest, InferenceResponse, RuntimeState};

/// Configuration for llama.cpp server subprocess.
#[derive(Debug, Clone)]
pub struct LlamaCppConfig {
    pub binary_path: PathBuf,
    pub host: String,
    pub port: u16,
    pub n_gpu_layers: u32,
    pub ctx_size: u32,
    pub startup_timeout_ms: u64,
}

impl Default for LlamaCppConfig {
    fn default() -> Self {
        Self {
            binary_path: PathBuf::from("llama-server"),
            host: "127.0.0.1".into(),
            port: 8081,
            n_gpu_layers: 0,
            ctx_size: 4096,
            startup_timeout_ms: 30_000,
        }
    }
}

/// llama.cpp server runtime via HTTP `/completion` API.
pub struct LlamaCppRuntime {
    config: LlamaCppConfig,
    client: reqwest::Client,
    process: Arc<Mutex<Option<Child>>>,
    model_path: Arc<Mutex<Option<PathBuf>>>,
    state: Arc<AtomicU32>,
}

impl LlamaCppRuntime {
    pub fn new(config: LlamaCppConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
            process: Arc::new(Mutex::new(None)),
            model_path: Arc::new(Mutex::new(None)),
            state: Arc::new(AtomicU32::new(RuntimeState::Unloaded as u32)),
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}:{}", self.config.host, self.config.port)
    }

    fn set_state(&self, state: RuntimeState) {
        self.state.store(state as u32, Ordering::SeqCst);
    }

    async fn start_server(&self, model_path: &Path) -> ModelResult<()> {
        self.set_state(RuntimeState::Loading);

        let mut child = Command::new(&self.config.binary_path)
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
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                self.set_state(RuntimeState::Error);
                ModelError::runtime(format!(
                    "failed to spawn {}: {e}",
                    self.config.binary_path.display()
                ))
            })?;

        *self.process.lock().await = Some(child);

        let deadline = Instant::now()
            + Duration::from_millis(self.config.startup_timeout_ms);
        while Instant::now() < deadline {
            if self.health().await.unwrap_or(false) {
                self.set_state(RuntimeState::Ready);
                info!(model = %model_path.display(), "llama.cpp server ready");
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        self.stop_server().await.ok();
        self.set_state(RuntimeState::Error);
        Err(ModelError::runtime("llama.cpp server startup timeout"))
    }

    async fn stop_server(&self) -> ModelResult<()> {
        if let Some(mut child) = self.process.lock().await.take() {
            child.kill().await.ok();
            child.wait().await.ok();
        }
        Ok(())
    }
}

#[async_trait]
impl InferenceRuntime for LlamaCppRuntime {
    fn state(&self) -> RuntimeState {
        match self.state.load(Ordering::SeqCst) {
            x if x == RuntimeState::Unloaded as u32 => RuntimeState::Unloaded,
            x if x == RuntimeState::Loading as u32 => RuntimeState::Loading,
            x if x == RuntimeState::Ready as u32 => RuntimeState::Ready,
            _ => RuntimeState::Error,
        }
    }

    async fn load_model(&mut self, model_path: &Path) -> ModelResult<()> {
        if !model_path.exists() {
            return Err(ModelError::invalid(format!(
                "model not found: {}",
                model_path.display()
            )));
        }

        self.unload().await.ok();
        self.start_server(model_path).await?;
        *self.model_path.lock().await = Some(model_path.to_path_buf());
        Ok(())
    }

    async fn unload(&mut self) -> ModelResult<()> {
        self.stop_server().await?;
        *self.model_path.lock().await = None;
        self.set_state(RuntimeState::Unloaded);
        Ok(())
    }

    async fn complete(&self, request: InferenceRequest) -> ModelResult<InferenceResponse> {
        if self.state() != RuntimeState::Ready {
            return Err(ModelError::runtime("runtime not ready"));
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
            .map_err(|e| ModelError::runtime(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ModelError::runtime(format!(
                "completion HTTP {}",
                response.status()
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ModelError::runtime(e.to_string()))?;

        let text = json
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let tokens = json
            .get("tokens_predicted")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        debug!(tokens, duration_ms = started.elapsed().as_millis(), "inference complete");

        Ok(InferenceResponse {
            text,
            tokens_predicted: tokens,
            duration_ms: started.elapsed().as_millis() as u64,
        })
    }

    async fn health(&self) -> ModelResult<bool> {
        let url = format!("{}/health", self.base_url());
        match self.client.get(&url).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(err) => {
                warn!(%err, "llama.cpp health check failed");
                Ok(false)
            }
        }
    }
}

impl Drop for LlamaCppRuntime {
    fn drop(&mut self) {
        if self.process.try_lock().is_ok() {
            debug!("llama.cpp runtime dropped");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = LlamaCppConfig::default();
        assert_eq!(cfg.port, 8081);
    }
}
