use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Supported model format (HTTP / API references only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelFormat {
    Api,
}

impl ModelFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Api => "api",
        }
    }
}

/// Model provider / acquisition channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelProvider {
    Ollama,
    Remote,
}

impl ModelProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::Remote => "remote",
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

    /// Default capabilities for remote / third-party HTTP providers.
    pub fn remote() -> Self {
        Self::default()
    }
}

/// Model acquisition source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelSource {
    Ollama {
        model: String,
        base_url: String,
    },
    Remote {
        provider: String,
        model: String,
        base_url: Option<String>,
        region: Option<String>,
    },
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

impl ModelEntry {
    /// Human-readable provider label (e.g. OpenAI instead of `remote`).
    pub fn display_provider(&self) -> String {
        if let Some(label) = self
            .metadata
            .get("providerLabel")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return label.to_string();
        }

        if self.provider == ModelProvider::Remote {
            let raw = self
                .metadata
                .get("remoteProvider")
                .and_then(|value| value.as_str())
                .or_else(|| match &self.source {
                    ModelSource::Remote { provider, .. } => Some(provider.as_str()),
                    _ => None,
                })
                .unwrap_or("remote");

            return remote_provider_display_name(raw);
        }

        if let Some(label) = publisher_display_name(&self.name) {
            return label;
        }

        match self.provider {
            ModelProvider::Ollama => "Ollama".into(),
            ModelProvider::Remote => "Remote".into(),
        }
    }

    /// Model identifier for display (remote API model id, or Ollama display name).
    pub fn display_model_name(&self) -> String {
        match &self.source {
            ModelSource::Remote { model, .. } => model.clone(),
            _ => self.name.clone(),
        }
    }
}

fn publisher_display_name(name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    if lower.contains("qwen") {
        Some("Qwen Team".into())
    } else if lower.contains("llama") {
        Some("Meta".into())
    } else if lower.contains("mistral") {
        Some("Mistral AI".into())
    } else {
        None
    }
}

fn remote_provider_display_name(provider: &str) -> String {
    match provider.trim().to_ascii_lowercase().as_str() {
        "openai" => "OpenAI".to_string(),
        "anthropic" => "Anthropic".to_string(),
        "gemini" | "google" => "Google".to_string(),
        "azure" => "Azure OpenAI".to_string(),
        "bedrock" | "aws_bedrock" => "Amazon Bedrock".to_string(),
        "openrouter" => "OpenRouter".to_string(),
        "nvidia" => "NVIDIA".to_string(),
        "custom" => "Custom".to_string(),
        other => {
            let mut label = other.to_string();
            if let Some(first) = label.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            label
        }
    }
}

/// Vault storage summary for desktop UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultStats {
    /// Active registered models (remote + Ollama HTTP).
    pub registered_count: usize,
    /// Ollama HTTP entries only.
    pub installed_local_count: usize,
    pub installed_bytes: u64,
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

/// GPU compute backend hint for hardware detection / tuning.
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
    pub fn primary_gpu(&self) -> Option<&GpuDevice> {
        self.gpus.first()
    }
}

/// Generic completion inference request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    pub prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl InferenceRequest {
    /// Single-string prompt for runtimes without separate system/user chat roles.
    pub fn effective_prompt(&self) -> String {
        match self.system.as_deref() {
            Some(system) if !system.trim().is_empty() => format!("{system}\n\n{}", self.prompt),
            _ => self.prompt.clone(),
        }
    }
}

/// Generic completion inference response.
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
