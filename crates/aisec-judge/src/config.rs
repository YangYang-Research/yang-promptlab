//! Judge provider configuration and hybrid mode settings.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::types::JudgeMode;

/// Local inference backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalProvider {
    Ollama,
    LlamaCpp,
}

impl Default for LocalProvider {
    fn default() -> Self {
        Self::Ollama
    }
}

/// Remote API provider selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteProvider {
    OpenAi,
    Anthropic,
    Gemini,
    OpenRouter,
}

impl Default for RemoteProvider {
    fn default() -> Self {
        Self::OpenAi
    }
}

/// Local model settings (Ollama, llama.cpp / GGUF).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalProviderSettings {
    pub provider: LocalProvider,
    #[serde(default = "default_ollama_url")]
    pub base_url: String,
    #[serde(default = "default_local_model")]
    pub model: String,
    #[serde(default)]
    pub model_path: Option<PathBuf>,
    /// Registered vault model id — resolved to path/tag at judge engine build time.
    #[serde(default)]
    pub vault_model_id: Option<String>,
    #[serde(default = "default_llama_binary")]
    pub llama_binary: String,
    #[serde(default = "default_llama_port")]
    pub llama_port: u16,
}

fn default_ollama_url() -> String {
    "http://127.0.0.1:11434".into()
}

fn default_local_model() -> String {
    "llama3".into()
}

fn default_llama_binary() -> String {
    "llama-server".into()
}

fn default_llama_port() -> u16 {
    8081
}

impl Default for LocalProviderSettings {
    fn default() -> Self {
        Self {
            provider: LocalProvider::Ollama,
            base_url: default_ollama_url(),
            model: default_local_model(),
            model_path: None,
            vault_model_id: None,
            llama_binary: default_llama_binary(),
            llama_port: default_llama_port(),
        }
    }
}

/// Remote API settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteProviderSettings {
    pub provider: RemoteProvider,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default = "default_remote_model")]
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub api_key_credential_id: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
}

fn default_remote_model() -> String {
    "gpt-4o-mini".into()
}

impl Default for RemoteProviderSettings {
    fn default() -> Self {
        Self {
            provider: RemoteProvider::OpenAi,
            base_url: None,
            model: default_remote_model(),
            api_key: String::new(),
            api_key_credential_id: None,
            api_key_env: Some("OPENAI_API_KEY".into()),
        }
    }
}

/// Full hybrid judge configuration persisted by the desktop app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeProviderConfig {
    pub mode: JudgeMode,
    #[serde(default)]
    pub local: LocalProviderSettings,
    #[serde(default)]
    pub remote: RemoteProviderSettings,
    #[serde(default = "default_threshold")]
    pub consensus_threshold: f32,
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f32,
    #[serde(default = "default_max_tokens")]
    pub llm_max_tokens: u32,
    #[serde(default = "default_temperature")]
    pub llm_temperature: f32,
}

fn default_threshold() -> f32 {
    0.55
}

fn default_min_confidence() -> f32 {
    0.45
}

fn default_max_tokens() -> u32 {
    512
}

fn default_temperature() -> f32 {
    0.1
}

impl Default for JudgeProviderConfig {
    fn default() -> Self {
        Self {
            mode: JudgeMode::Deterministic,
            local: LocalProviderSettings::default(),
            remote: RemoteProviderSettings::default(),
            consensus_threshold: default_threshold(),
            min_confidence: default_min_confidence(),
            llm_max_tokens: default_max_tokens(),
            llm_temperature: default_temperature(),
        }
    }
}

impl JudgeProviderConfig {
    pub fn to_engine_config(&self) -> crate::types::JudgeConfig {
        crate::types::JudgeConfig {
            mode: self.mode,
            enable_rules: true,
            enable_regex: true,
            enable_llm: matches!(
                self.mode,
                JudgeMode::LocalLlm | JudgeMode::RemoteLlm | JudgeMode::Consensus
            ),
            consensus_threshold: self.consensus_threshold,
            min_confidence: self.min_confidence,
            llm_max_tokens: self.llm_max_tokens,
            llm_temperature: self.llm_temperature,
        }
    }

    pub fn resolved_api_key(&self) -> Option<String> {
        if !self.remote.api_key.trim().is_empty() {
            return Some(self.remote.api_key.trim().to_string());
        }
        self.remote
            .api_key_env
            .as_deref()
            .and_then(|name| std::env::var(name).ok())
            .filter(|v| !v.trim().is_empty())
    }
}

/// Connectivity / smoke-test outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeConnectivityResult {
    pub ok: bool,
    pub provider: String,
    pub model: String,
    pub latency_ms: u64,
    pub message: String,
    #[serde(default)]
    pub sample_response: Option<String>,
}
