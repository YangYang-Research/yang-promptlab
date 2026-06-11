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
    pub resumed: bool,
    pub updated_at: OffsetDateTime,
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

/// Runtime load state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeState {
    Unloaded,
    Loading,
    Ready,
    Error,
}
