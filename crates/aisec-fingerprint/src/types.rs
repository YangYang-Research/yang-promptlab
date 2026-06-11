use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Known AI inference provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiProvider {
    OpenAi,
    Anthropic,
    Gemini,
    Bedrock,
    AzureOpenAi,
    Ollama,
    LiteLlm,
    Vllm,
}

impl AiProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
            Self::Bedrock => "bedrock",
            Self::AzureOpenAi => "azure_openai",
            Self::Ollama => "ollama",
            Self::LiteLlm => "litellm",
            Self::Vllm => "vllm",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Gemini => "Google Gemini",
            Self::Bedrock => "AWS Bedrock",
            Self::AzureOpenAi => "Azure OpenAI",
            Self::Ollama => "Ollama",
            Self::LiteLlm => "LiteLLM",
            Self::Vllm => "vLLM",
        }
    }

    pub fn all() -> &'static [AiProvider] {
        use AiProvider::*;
        &[
            OpenAi,
            Anthropic,
            Gemini,
            Bedrock,
            AzureOpenAi,
            Ollama,
            LiteLlm,
            Vllm,
        ]
    }
}

/// HTTP observation used for fingerprinting.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FingerprintInput {
    pub url: String,
    pub method: Option<String>,
    pub status: Option<u16>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl FingerprintInput {
    pub fn from_parts(
        url: impl Into<String>,
        status: Option<u16>,
        headers: HashMap<String, String>,
        body: Option<String>,
    ) -> Self {
        Self {
            url: url.into(),
            method: None,
            status,
            headers,
            body,
        }
    }
}

/// A matched detection signal contributing to confidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchedSignal {
    pub provider: AiProvider,
    pub rule_id: String,
    pub description: String,
    pub weight: f32,
}

/// Provider fingerprint result with confidence score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderFingerprint {
    pub provider: AiProvider,
    pub confidence: f32,
    pub signals: Vec<MatchedSignal>,
    pub inferred_api_style: ApiStyle,
    pub suggested_method: Option<String>,
}

/// API compatibility style inferred from fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiStyle {
    OpenAiCompatible,
    AnthropicMessages,
    GeminiGenerateContent,
    BedrockInvoke,
    OllamaNative,
    Unknown,
}

/// Aggregated fingerprint report for an endpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FingerprintReport {
    pub url: String,
    pub matches: Vec<ProviderFingerprint>,
    pub primary: Option<ProviderFingerprint>,
    pub analyzed_at: OffsetDateTime,
}

impl FingerprintReport {
    pub fn best_match(&self) -> Option<&ProviderFingerprint> {
        self.primary.as_ref().or_else(|| self.matches.first())
    }
}

/// Minimum confidence to include a provider in results.
pub const DEFAULT_CONFIDENCE_THRESHOLD: f32 = 0.45;
