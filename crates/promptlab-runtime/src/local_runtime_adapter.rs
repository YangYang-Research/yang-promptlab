//! Local runtime adapter stub — embedded libllama / GGUF has been removed.
//! API shapes are retained so desktop and inference host still compile.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::info;

use crate::error::{RuntimeError, RuntimeResult};
use crate::hardware::RuntimeHardwareProfile;

const UNAVAILABLE: &str =
    "local embedded runtime has been removed — configure a remote AI provider or Ollama over HTTP";

/// Resolved runtime compute backend (legacy local-runtime field).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeBackend {
    Cpu,
    Cuda,
    Metal,
    Vulkan,
}

impl RuntimeBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Cuda => "cuda",
            Self::Metal => "metal",
            Self::Vulkan => "vulkan",
        }
    }
}

/// User-selectable GPU/CPU backend (retained for config compatibility).
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

#[derive(Debug, Clone)]
pub struct InferRequest {
    pub prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

#[derive(Debug, Clone)]
pub struct InferResponse {
    pub text: String,
    pub tokens_predicted: u32,
    pub duration_ms: u64,
}

/// Stub adapter — always reports local runtime unavailable.
pub struct LocalRuntimeAdapter {
    backend: GfxBackend,
    n_gpu_layers: u32,
    model_path: Mutex<Option<PathBuf>>,
    initialized: AtomicBool,
    model_loaded: AtomicBool,
    capabilities: Mutex<LocalRuntimeCapabilities>,
}

impl LocalRuntimeAdapter {
    pub fn new(_n_gpu_layers: u32, backend: GfxBackend) -> Self {
        Self {
            backend,
            n_gpu_layers: _n_gpu_layers,
            model_path: Mutex::new(None),
            initialized: AtomicBool::new(false),
            model_loaded: AtomicBool::new(false),
            capabilities: Mutex::new(LocalRuntimeCapabilities::default()),
        }
    }

    pub fn backend(&self) -> GfxBackend {
        self.backend
    }

    pub fn n_gpu_layers(&self) -> u32 {
        self.n_gpu_layers
    }

    pub fn runtime_available(&self) -> bool {
        false
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }

    pub fn is_loaded(&self) -> bool {
        false
    }

    pub async fn is_loaded_async(&self) -> bool {
        false
    }

    pub async fn initialize(&self) -> RuntimeResult<()> {
        if self.initialized.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        info!("local runtime stub initialized (embedded libllama unavailable)");
        Ok(())
    }

    pub fn set_backend(&mut self, backend: GfxBackend, profile: Option<&RuntimeHardwareProfile>) {
        self.backend = backend;
        self.n_gpu_layers = n_gpu_layers_for_backend(resolve_backend(backend, profile), profile);
        self.initialized.store(false, Ordering::SeqCst);
        self.model_loaded.store(false, Ordering::SeqCst);
    }

    pub fn set_n_gpu_layers(&mut self, n_gpu_layers: u32) {
        self.n_gpu_layers = n_gpu_layers;
    }

    pub async fn load_model(&self, _model_path: &Path) -> RuntimeResult<()> {
        Err(RuntimeError::BackendUnavailable(UNAVAILABLE.into()))
    }

    pub async fn unload(&self) -> RuntimeResult<()> {
        *self.model_path.lock().await = None;
        self.model_loaded.store(false, Ordering::SeqCst);
        Ok(())
    }

    pub async fn infer(&self, _request: InferRequest) -> RuntimeResult<InferResponse> {
        Err(RuntimeError::BackendUnavailable(UNAVAILABLE.into()))
    }

    pub async fn health(&self) -> RuntimeResult<bool> {
        Ok(false)
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

pub fn default_n_gpu_layers(profile: Option<&RuntimeHardwareProfile>) -> u32 {
    let backend = resolve_backend(GfxBackend::Auto, profile);
    n_gpu_layers_for_backend(backend, profile)
}

/// Retained helper for callers that previously built a local model config.
pub fn default_model_config(profile: Option<&RuntimeHardwareProfile>) -> u32 {
    default_n_gpu_layers(profile)
}
