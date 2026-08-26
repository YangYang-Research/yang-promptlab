use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{InferenceError, InferenceResult};

/// Inference route: third-party HTTP API or rule-based deterministic mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InferenceMode {
    ThirdParty,
    /// Rule-based evaluation only — no LLM.
    Deterministic,
}

impl InferenceMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ThirdParty => "third_party",
            Self::Deterministic => "deterministic",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "third_party" | "third-party" | "cloud" | "remote" => Some(Self::ThirdParty),
            "deterministic" | "rules" => Some(Self::Deterministic),
            _ => None,
        }
    }
}

/// Provider identifier for the active inference route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InferenceProvider {
    OpenAi,
    Anthropic,
    Gemini,
    OpenRouter,
    Nvidia,
    Azure,
    Bedrock,
    Ollama,
    Deterministic,
}

impl InferenceProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
            Self::OpenRouter => "openrouter",
            Self::Nvidia => "nvidia",
            Self::Azure => "azure",
            Self::Bedrock => "bedrock",
            Self::Ollama => "ollama",
            Self::Deterministic => "deterministic",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "anthropic" => Self::Anthropic,
            "gemini" | "google" => Self::Gemini,
            "openrouter" => Self::OpenRouter,
            "nvidia" => Self::Nvidia,
            "azure" => Self::Azure,
            "bedrock" | "aws_bedrock" => Self::Bedrock,
            "ollama" => Self::Ollama,
            "deterministic" => Self::Deterministic,
            _ => Self::OpenAi,
        }
    }
}

/// Runtime health snapshot stored with configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHealth {
    pub ok: bool,
    pub message: String,
    pub latency_ms: Option<u64>,
    pub checked_at: Option<String>,
}

/// Single unified AI runtime configuration — the only persisted inference settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiRuntimeConfiguration {
    pub mode: InferenceMode,
    pub provider: InferenceProvider,
    pub runtime: String,
    pub model: String,
    pub selected_model_id: Option<String>,
    pub status: String,
    pub health: RuntimeHealth,
    pub temperature: f32,
    pub max_tokens: u32,
    pub timeout_secs: u64,
    pub streaming: bool,
    pub context_length: Option<u32>,
    pub initialized: bool,
}

impl Default for AiRuntimeConfiguration {
    fn default() -> Self {
        Self {
            mode: InferenceMode::ThirdParty,
            provider: InferenceProvider::OpenAi,
            runtime: "cloud".into(),
            model: String::new(),
            selected_model_id: None,
            status: "ready".into(),
            health: RuntimeHealth::default(),
            temperature: 0.1,
            max_tokens: 512,
            timeout_secs: 120,
            streaming: false,
            context_length: None,
            initialized: true,
        }
    }
}

pub fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("ai_runtime_config.json")
}

pub async fn load_config(data_dir: &Path) -> InferenceResult<AiRuntimeConfiguration> {
    let path = config_path(data_dir);
    if !path.is_file() {
        return Ok(AiRuntimeConfiguration::default());
    }
    let raw = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| InferenceError::Internal(e.to_string()))?;
    if raw.trim().is_empty() {
        return Ok(AiRuntimeConfiguration::default());
    }
    serde_json::from_str(&raw).map_err(|e| InferenceError::Serialization(e.to_string()))
}

pub async fn save_config(data_dir: &Path, config: &AiRuntimeConfiguration) -> InferenceResult<()> {
    let path = config_path(data_dir);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| InferenceError::Internal(e.to_string()))?;
    }
    let raw = serde_json::to_string_pretty(config)
        .map_err(|e| InferenceError::Serialization(e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    tokio::fs::write(&tmp, raw)
        .await
        .map_err(|e| InferenceError::Internal(e.to_string()))?;
    tokio::fs::rename(&tmp, &path)
        .await
        .map_err(|e| InferenceError::Internal(e.to_string()))?;
    Ok(())
}
