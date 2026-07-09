//! Embedded libllama runtime — in-process GGUF inference (no subprocess, no HTTP).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use aisec_models::runtime::{InferenceRuntime, LlamaInProcessRuntime, LlamaModelConfig};
use aisec_models::types::{InferenceRequest, RuntimeState};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::info;

use crate::error::{RuntimeError, RuntimeResult};
use crate::hardware::RuntimeHardwareProfile;
use crate::manifest::RuntimeBackend;
use crate::runtime::gguf::{detect_quantization, GgufQuantization};

/// User-selectable GPU/CPU backend for embedded libllama.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GfxBackend {
    Auto,
    Cuda,
    Metal,
    Vulkan,
    Cpu,
}

impl GfxBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Cuda => "cuda",
            Self::Metal => "metal",
            Self::Vulkan => "vulkan",
            Self::Cpu => "cpu",
        }
    }
}

/// Capabilities derived from the currently loaded model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalRuntimeCapabilities {
    pub supports_chat: bool,
    pub supports_embedding: bool,
    pub supports_streaming: bool,
    pub supports_vision: bool,
    pub supports_json: bool,
    pub supports_tools: bool,
    pub supports_thinking: bool,
    pub max_context: u32,
    pub embedding_dimensions: Option<u32>,
}

impl Default for LocalRuntimeCapabilities {
    fn default() -> Self {
        Self {
            supports_chat: true,
            supports_embedding: false,
            supports_streaming: true,
            supports_vision: false,
            supports_json: true,
            supports_tools: false,
            supports_thinking: false,
            max_context: 4096,
            embedding_dimensions: None,
        }
    }
}

/// Inference request for embedded libllama.
#[derive(Debug, Clone)]
pub struct InferRequest {
    pub prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

/// Inference response from embedded libllama.
#[derive(Debug, Clone)]
pub struct InferResponse {
    pub text: String,
    pub tokens_predicted: u32,
    pub duration_ms: u64,
    pub quantization: Option<GgufQuantization>,
}

/// Owns all native llama resources — no other module may call libllama directly.
pub struct LocalRuntimeAdapter {
    backend: GfxBackend,
    config: LlamaModelConfig,
    runtime: Mutex<LlamaInProcessRuntime>,
    model_path: Mutex<Option<PathBuf>>,
    initialized: AtomicBool,
    model_loaded: AtomicBool,
    capabilities: Mutex<LocalRuntimeCapabilities>,
}

impl LocalRuntimeAdapter {
    pub fn new(config: LlamaModelConfig, backend: GfxBackend) -> Self {
        Self {
            backend,
            config: config.clone(),
            runtime: Mutex::new(LlamaInProcessRuntime::new(config)),
            model_path: Mutex::new(None),
            initialized: AtomicBool::new(false),
            model_loaded: AtomicBool::new(false),
            capabilities: Mutex::new(LocalRuntimeCapabilities::default()),
        }
    }

    pub fn backend(&self) -> GfxBackend {
        self.backend
    }

    pub fn config(&self) -> &LlamaModelConfig {
        &self.config
    }

    /// Embedded libllama is always available when compiled in.
    pub fn runtime_available(&self) -> bool {
        true
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }

    pub fn is_loaded(&self) -> bool {
        self.model_loaded.load(Ordering::SeqCst)
    }

    pub async fn is_loaded_async(&self) -> bool {
        if !self.model_loaded.load(Ordering::SeqCst) {
            return false;
        }
        let rt = self.runtime.lock().await;
        rt.state() == RuntimeState::Ready
    }

    pub async fn initialize(&self) -> RuntimeResult<()> {
        if self.initialized.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        info!(backend = %self.backend.as_str(), "embedded libllama runtime initialized");
        Ok(())
    }

    pub fn set_backend(&mut self, backend: GfxBackend, profile: Option<&RuntimeHardwareProfile>) {
        self.backend = backend;
        self.config.n_gpu_layers = n_gpu_layers_for_backend(resolve_backend(backend, profile), profile);
        let rt = LlamaInProcessRuntime::new(self.config.clone());
        self.runtime = Mutex::new(rt);
        self.initialized.store(false, Ordering::SeqCst);
        self.model_loaded.store(false, Ordering::SeqCst);
    }

    pub fn set_n_gpu_layers(&mut self, n_gpu_layers: u32) {
        self.config.n_gpu_layers = n_gpu_layers;
    }

    pub async fn load_model(&self, model_path: &Path) -> RuntimeResult<()> {
        if self.is_loaded() {
            if let Some(loaded) = self.loaded_model_path().await {
                if crate::paths::same_paths(&loaded, model_path) {
                    return Ok(());
                }
            }
        }

        self.initialize().await?;
        let quant = detect_quantization(model_path);
        let mut caps = LocalRuntimeCapabilities::default();
        caps.max_context = self.config.ctx_size;
        *self.capabilities.lock().await = caps;

        let mut rt = self.runtime.lock().await;
        rt.load_model(model_path)
            .await
            .map_err(RuntimeError::from)?;
        *self.model_path.lock().await = Some(model_path.to_path_buf());
        self.model_loaded.store(true, Ordering::SeqCst);
        info!(
            model = %model_path.display(),
            quant = quant.as_str(),
            "GGUF model loaded via embedded libllama"
        );
        Ok(())
    }

    pub async fn unload(&self) -> RuntimeResult<()> {
        let mut rt = self.runtime.lock().await;
        rt.unload().await.map_err(RuntimeError::from)?;
        *self.model_path.lock().await = None;
        self.model_loaded.store(false, Ordering::SeqCst);
        Ok(())
    }

    pub async fn infer(&self, request: InferRequest) -> RuntimeResult<InferResponse> {
        let rt = self.runtime.lock().await;
        if rt.state() != RuntimeState::Ready {
            return Err(RuntimeError::ModelNotLoaded);
        }
        let model_path = self.model_path.lock().await.clone();
        let quant = model_path.as_deref().map(detect_quantization);

        let response = rt
            .complete(InferenceRequest {
                system: None,
                prompt: request.prompt,
                max_tokens: request.max_tokens,
                temperature: request.temperature,
            })
            .await
            .map_err(RuntimeError::from)?;

        Ok(InferResponse {
            text: response.text,
            tokens_predicted: response.tokens_predicted,
            duration_ms: response.duration_ms,
            quantization: quant,
        })
    }

    pub async fn health(&self) -> RuntimeResult<bool> {
        let rt = self.runtime.lock().await;
        rt.health().await.map_err(RuntimeError::from)
    }

    pub async fn loaded_model_path(&self) -> Option<PathBuf> {
        self.model_path.lock().await.clone()
    }

    pub async fn capabilities(&self) -> LocalRuntimeCapabilities {
        self.capabilities.lock().await.clone()
    }

    pub async fn shutdown(&self) -> RuntimeResult<()> {
        self.unload().await
    }
}

pub fn resolve_backend(
    backend: GfxBackend,
    profile: Option<&RuntimeHardwareProfile>,
) -> RuntimeBackend {
    match backend {
        GfxBackend::Cuda => RuntimeBackend::Cuda,
        GfxBackend::Metal => RuntimeBackend::Metal,
        GfxBackend::Vulkan => RuntimeBackend::Vulkan,
        GfxBackend::Cpu => RuntimeBackend::Cpu,
        GfxBackend::Auto => {
            let Some(p) = profile else {
                return RuntimeBackend::Cpu;
            };
            if p.metal {
                RuntimeBackend::Metal
            } else if p.cuda {
                RuntimeBackend::Cuda
            } else if p.vulkan {
                RuntimeBackend::Vulkan
            } else {
                RuntimeBackend::Cpu
            }
        }
    }
}

pub fn n_gpu_layers_for_backend(
    backend: RuntimeBackend,
    profile: Option<&RuntimeHardwareProfile>,
) -> u32 {
    match backend {
        RuntimeBackend::Cpu => 0,
        _ => profile
            .and_then(|p| p.vram_bytes)
            .map(|vram| {
                if vram >= 8 * 1024 * 1024 * 1024 {
                    99
                } else if vram >= 4 * 1024 * 1024 * 1024 {
                    35
                } else {
                    20
                }
            })
            .unwrap_or(0),
    }
}

pub fn default_model_config(profile: Option<&RuntimeHardwareProfile>) -> LlamaModelConfig {
    let backend = resolve_backend(GfxBackend::Auto, profile);
    let mut config = LlamaModelConfig::default();
    config.n_gpu_layers = n_gpu_layers_for_backend(backend, profile);
    config
}
