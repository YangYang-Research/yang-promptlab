//! In-process llama.cpp GGUF runtime.
//!
//! Loads a GGUF model **inside the process** via llama.cpp FFI bindings
//! (`llama-cpp-2`) and executes real inference — no subprocess, no HTTP, no
//! mocked output.
//!
//! ## Threading model
//!
//! llama.cpp model/context objects are not `Send`/`Sync`, so all FFI objects are
//! confined to a single dedicated worker thread. The runtime communicates with
//! that thread over channels, which keeps [`LlamaInProcessRuntime`] `Send + Sync`
//! (as required by [`InferenceRuntime`]) while the model stays on one thread.

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel, Special};
use llama_cpp_2::sampling::LlamaSampler;
use tokio::sync::oneshot;
use tracing::{debug, info};

use crate::error::{ModelError, ModelResult};
use crate::runtime::InferenceRuntime;
use crate::types::{InferenceRequest, InferenceResponse, RuntimeState};

/// Configuration for the in-process llama.cpp runtime.
#[derive(Debug, Clone)]
pub struct LlamaModelConfig {
    /// Layers to offload to GPU (0 = CPU only).
    pub n_gpu_layers: u32,
    /// Context window size (tokens).
    pub ctx_size: u32,
    /// CPU threads used for inference.
    pub n_threads: u32,
}

impl Default for LlamaModelConfig {
    fn default() -> Self {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(4);
        Self {
            n_gpu_layers: 0,
            ctx_size: 4096,
            n_threads: threads,
        }
    }
}

enum Command {
    Generate {
        request: InferenceRequest,
        reply: oneshot::Sender<ModelResult<InferenceResponse>>,
    },
    Shutdown,
}

struct Worker {
    tx: std::sync::mpsc::Sender<Command>,
    handle: Option<std::thread::JoinHandle<()>>,
}

/// In-process GGUF inference runtime backed by llama.cpp.
pub struct LlamaInProcessRuntime {
    config: LlamaModelConfig,
    state: Arc<AtomicU32>,
    model_path: std::sync::Mutex<Option<PathBuf>>,
    worker: std::sync::Mutex<Option<Worker>>,
}

impl LlamaInProcessRuntime {
    pub fn new(config: LlamaModelConfig) -> Self {
        Self {
            config,
            state: Arc::new(AtomicU32::new(RuntimeState::Unloaded as u32)),
            model_path: std::sync::Mutex::new(None),
            worker: std::sync::Mutex::new(None),
        }
    }

    pub fn config(&self) -> &LlamaModelConfig {
        &self.config
    }

    fn set_state(&self, state: RuntimeState) {
        self.state.store(state as u32, Ordering::SeqCst);
    }

    fn sender(&self) -> Option<std::sync::mpsc::Sender<Command>> {
        self.worker
            .lock()
            .unwrap()
            .as_ref()
            .map(|w| w.tx.clone())
    }
}

#[async_trait]
impl InferenceRuntime for LlamaInProcessRuntime {
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

        if self.state() == RuntimeState::Ready {
            if let Some(current) = self.model_path.lock().unwrap().as_ref() {
                if paths_equal(current, model_path) {
                    return Ok(());
                }
            }
        }

        // Tear down any previously loaded model before loading a different GGUF.
        self.unload().await.ok();
        self.set_state(RuntimeState::Loading);

        let (tx, rx) = std::sync::mpsc::channel::<Command>();
        let (ready_tx, ready_rx) = oneshot::channel::<ModelResult<()>>();
        let cfg = self.config.clone();
        let path = model_path.to_path_buf();

        let handle = std::thread::Builder::new()
            .name("promptlab-llama".into())
            .spawn(move || worker_loop(path, cfg, rx, ready_tx))
            .map_err(|e| ModelError::runtime(format!("failed to spawn llama worker: {e}")))?;

        match ready_rx.await {
            Ok(Ok(())) => {
                *self.worker.lock().unwrap() = Some(Worker {
                    tx,
                    handle: Some(handle),
                });
                *self.model_path.lock().unwrap() = Some(model_path.to_path_buf());
                self.set_state(RuntimeState::Ready);
                info!(model = %model_path.display(), "in-process llama.cpp model loaded");
                Ok(())
            }
            Ok(Err(err)) => {
                let _ = handle.join();
                self.set_state(RuntimeState::Error);
                Err(err)
            }
            Err(_) => {
                let _ = handle.join();
                self.set_state(RuntimeState::Error);
                Err(ModelError::runtime("llama worker exited during model load"))
            }
        }
    }

    async fn unload(&mut self) -> ModelResult<()> {
        let worker = self.worker.lock().unwrap().take();
        if let Some(mut worker) = worker {
            let _ = worker.tx.send(Command::Shutdown);
            // Joining guarantees the model + backend are freed before any reload.
            if let Some(handle) = worker.handle.take() {
                let _ = handle.join();
            }
        }
        *self.model_path.lock().unwrap() = None;
        self.set_state(RuntimeState::Unloaded);
        Ok(())
    }

    async fn complete(&self, request: InferenceRequest) -> ModelResult<InferenceResponse> {
        if self.state() != RuntimeState::Ready {
            return Err(ModelError::runtime("runtime not ready"));
        }
        let tx = self
            .sender()
            .ok_or_else(|| ModelError::runtime("model not loaded"))?;

        let (reply_tx, reply_rx) = oneshot::channel();
        tx.send(Command::Generate {
            request,
            reply: reply_tx,
        })
        .map_err(|_| ModelError::runtime("llama worker unavailable"))?;

        reply_rx
            .await
            .map_err(|_| ModelError::runtime("llama worker dropped the reply"))?
    }

    async fn health(&self) -> ModelResult<bool> {
        Ok(self.state() == RuntimeState::Ready)
    }
}

impl Drop for LlamaInProcessRuntime {
    fn drop(&mut self) {
        if let Some(mut worker) = self.worker.lock().unwrap().take() {
            let _ = worker.tx.send(Command::Shutdown);
            if let Some(handle) = worker.handle.take() {
                let _ = handle.join();
            }
        }
    }
}

/// Dedicated worker thread: owns the llama backend, model, and contexts.
fn worker_loop(
    model_path: PathBuf,
    cfg: LlamaModelConfig,
    rx: std::sync::mpsc::Receiver<Command>,
    ready: oneshot::Sender<ModelResult<()>>,
) {
    let backend = match LlamaBackend::init() {
        Ok(b) => b,
        Err(e) => {
            let _ = ready.send(Err(ModelError::runtime(format!(
                "llama backend init failed: {e}"
            ))));
            return;
        }
    };

    let model_params = LlamaModelParams::default().with_n_gpu_layers(cfg.n_gpu_layers);
    let model = match LlamaModel::load_from_file(&backend, &model_path, &model_params) {
        Ok(m) => m,
        Err(e) => {
            let _ = ready.send(Err(ModelError::runtime(format!(
                "failed to load GGUF model {}: {e}",
                model_path.display()
            ))));
            return;
        }
    };

    if ready.send(Ok(())).is_err() {
        return; // loader gave up
    }

    while let Ok(cmd) = rx.recv() {
        match cmd {
            Command::Shutdown => break,
            Command::Generate { request, reply } => {
                let result = generate(&backend, &model, &cfg, &request);
                let _ = reply.send(result);
            }
        }
    }
    debug!("llama worker shut down");
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    std::fs::canonicalize(a)
        .ok()
        .zip(std::fs::canonicalize(b).ok())
        .map(|(a, b)| a == b)
        .unwrap_or_else(|| a == b)
}

/// Run a single real inference pass (prefill + token-by-token generation).
fn generate(
    backend: &LlamaBackend,
    model: &LlamaModel,
    cfg: &LlamaModelConfig,
    request: &InferenceRequest,
) -> ModelResult<InferenceResponse> {
    let started = Instant::now();

    let ctx_size = cfg.ctx_size.max(256);
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(ctx_size))
        .with_n_threads(cfg.n_threads as i32)
        .with_n_threads_batch(cfg.n_threads as i32);

    let mut ctx = model
        .new_context(backend, ctx_params)
        .map_err(|e| ModelError::runtime(format!("failed to create context: {e}")))?;

    let tokens = model
        .str_to_token(&request.effective_prompt(), AddBos::Always)
        .map_err(|e| ModelError::runtime(format!("tokenization failed: {e}")))?;

    let n_prompt = tokens.len();
    if n_prompt == 0 {
        return Err(ModelError::invalid("empty prompt"));
    }

    let batch_capacity = n_prompt.max(512);
    let mut batch = LlamaBatch::new(batch_capacity, 1);
    for (i, token) in tokens.iter().enumerate() {
        let is_last = i == n_prompt - 1;
        batch
            .add(*token, i as i32, &[0], is_last)
            .map_err(|e| ModelError::runtime(format!("batch add failed: {e}")))?;
    }
    ctx.decode(&mut batch)
        .map_err(|e| ModelError::runtime(format!("prefill decode failed: {e}")))?;

    let mut sampler = if request.temperature <= 0.0 {
        LlamaSampler::chain_simple([LlamaSampler::greedy()])
    } else {
        LlamaSampler::chain_simple([
            LlamaSampler::temp(request.temperature),
            LlamaSampler::dist(0),
        ])
    };

    let max_tokens = request.max_tokens.max(1);
    let mut out_bytes: Vec<u8> = Vec::new();
    let mut produced: u32 = 0;
    let mut pos = n_prompt as i32;

    for _ in 0..max_tokens {
        let token = sampler.sample(&ctx, batch.n_tokens() - 1);
        if model.is_eog_token(token) {
            break;
        }

        let bytes = model
            .token_to_bytes(token, Special::Plaintext)
            .map_err(|e| ModelError::runtime(format!("detokenize failed: {e}")))?;
        out_bytes.extend_from_slice(&bytes);
        produced += 1;

        batch.clear();
        batch
            .add(token, pos, &[0], true)
            .map_err(|e| ModelError::runtime(format!("batch add failed: {e}")))?;
        ctx.decode(&mut batch)
            .map_err(|e| ModelError::runtime(format!("decode failed: {e}")))?;
        pos += 1;
    }

    let text = String::from_utf8_lossy(&out_bytes).into_owned();
    let duration_ms = started.elapsed().as_millis() as u64;
    debug!(produced, duration_ms, "in-process inference complete");

    Ok(InferenceResponse {
        text,
        tokens_predicted: produced,
        duration_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_uses_cpu() {
        let cfg = LlamaModelConfig::default();
        assert_eq!(cfg.n_gpu_layers, 0);
        assert!(cfg.ctx_size >= 256);
        assert!(cfg.n_threads >= 1);
    }
}
