use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Supported on-disk model format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelFormat {
    Gguf,
}

impl ModelFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gguf => "gguf",
        }
    }

    pub fn from_path(path: &std::path::Path) -> Option<Self> {
        path.extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase)
            .and_then(|ext| if ext == "gguf" { Some(Self::Gguf) } else { None })
    }
}

/// Model provider / acquisition channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelProvider {
    Ollama,
    HuggingFace,
    Gguf,
}

impl ModelProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::HuggingFace => "huggingface",
            Self::Gguf => "gguf",
        }
    }
}

/// Supported inference capabilities for a registered model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCapabilities {
    pub chat: bool,
    pub completion: bool,
    pub embeddings: bool,
}

impl Default for ModelCapabilities {
    fn default() -> Self {
        Self {
            chat: true,
            completion: true,
            embeddings: false,
        }
    }
}

impl ModelCapabilities {
    pub fn ollama() -> Self {
        Self {
            chat: true,
            completion: true,
            embeddings: true,
        }
    }

    pub fn gguf() -> Self {
        Self {
            chat: true,
            completion: true,
            embeddings: false,
        }
    }
}

/// Model acquisition source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelSource {
    Local { path: PathBuf },
    HuggingFace {
        repo: String,
        filename: String,
        revision: Option<String>,
    },
    Ollama {
        model: String,
        base_url: String,
    },
}

/// Download job lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    Pending,
    Downloading,
    Paused,
    Completed,
    Failed,
    Verified,
}

/// Registered model metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub name: String,
    pub format: ModelFormat,
    pub provider: ModelProvider,
    pub version: String,
    pub capabilities: ModelCapabilities,
    pub source: ModelSource,
    pub file_path: PathBuf,
    pub size_bytes: Option<u64>,
    pub checksum_sha256: Option<String>,
    pub verified: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Curated or discovered model available for installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCatalogEntry {
    pub id: String,
    pub name: String,
    pub provider: ModelProvider,
    pub version: String,
    pub description: String,
    pub purpose: String,
    pub recommended: bool,
    pub size_bytes: Option<u64>,
    pub quant: Option<String>,
    pub capabilities: ModelCapabilities,
    pub repo: Option<String>,
    pub filename: Option<String>,
    #[serde(default)]
    pub engine: String,
    #[serde(default)]
    pub format: String,
    pub download_url: Option<String>,
    pub sha256: Option<String>,
    pub size_label: Option<String>,
}

/// HuggingFace download request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HuggingFaceDownloadRequest {
    pub name: String,
    pub repo: String,
    pub filename: String,
    pub revision: Option<String>,
    pub expected_sha256: Option<String>,
    pub expected_size_bytes: Option<u64>,
}

/// Download progress snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub model_id: String,
    pub status: DownloadStatus,
    pub url: String,
    pub destination: PathBuf,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    #[serde(default)]
    pub speed_bytes_per_sec: Option<f64>,
    #[serde(default)]
    pub eta_seconds: Option<u64>,
    pub resumed: bool,
    pub updated_at: OffsetDateTime,
}

/// Vault storage summary for desktop UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultStats {
    pub model_count: usize,
    pub installed_bytes: u64,
    pub disk_usage_bytes: u64,
    pub vault_path: PathBuf,
}

/// SHA256 verification outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationResult {
    pub file_path: PathBuf,
    pub expected_sha256: Option<String>,
    pub actual_sha256: String,
    pub size_bytes: u64,
    pub valid: bool,
}

/// GPU compute backend hint for llama.cpp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuBackend {
    None,
    Metal,
    Cuda,
    Vulkan,
    Rocm,
}

/// Detected GPU device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuDevice {
    pub name: String,
    pub vendor: Option<String>,
    pub vram_bytes: Option<u64>,
    pub backend: GpuBackend,
}

/// Host hardware profile for runtime tuning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub os: String,
    pub arch: String,
    pub cpu_cores: usize,
    pub total_memory_bytes: u64,
    pub gpus: Vec<GpuDevice>,
}

impl HardwareProfile {
    pub fn recommended_gpu_layers(&self) -> u32 {
        if self.gpus.is_empty() {
            0
        } else {
            35
        }
    }

    pub fn primary_gpu(&self) -> Option<&GpuDevice> {
        self.gpus.first()
    }
}

/// llama.cpp inference request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRequest {
    pub prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

/// llama.cpp inference response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResponse {
    pub text: String,
    pub tokens_predicted: u32,
    pub duration_ms: u64,
}

/// Chat message for multi-turn inference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Chat inference request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub max_tokens: u32,
    pub temperature: f32,
}

/// Chat inference response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub message: ChatMessage,
    pub tokens_predicted: u32,
    pub duration_ms: u64,
}

/// Embedding inference request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    pub input: String,
}

/// Embedding inference response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub vector: Vec<f32>,
    pub dimensions: usize,
    pub duration_ms: u64,
}

/// Runtime load state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeState {
    Unloaded,
    Loading,
    Ready,
    Error,
}
