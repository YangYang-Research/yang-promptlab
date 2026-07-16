//! Judge provider configuration and hybrid mode settings.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::types::JudgeMode;
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
    Nvidia,
    Azure,
    Bedrock,
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
    #[serde(default)]
    pub aws_secret_access_key: String,
    #[serde(default)]
    pub aws_secret_access_key_credential_id: Option<String>,
    #[serde(default)]
    pub aws_region: Option<String>,
    #[serde(default)]
    pub aws_session_token: String,
    #[serde(default)]
    pub aws_session_token_credential_id: Option<String>,
}

fn default_remote_model() -> String {
    String::new()
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
            aws_secret_access_key: String::new(),
            aws_secret_access_key_credential_id: None,
            aws_region: None,
            aws_session_token: String::new(),
            aws_session_token_credential_id: None,
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
            mode: JudgeMode::LocalLlm,
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

    pub fn resolved_aws_secret_access_key(&self) -> Option<String> {
        if !self.remote.aws_secret_access_key.trim().is_empty() {
            return Some(self.remote.aws_secret_access_key.trim().to_string());
        }
        if self.remote.aws_secret_access_key_credential_id.is_some() {
            return None;
        }
        std::env::var("AWS_SECRET_ACCESS_KEY")
            .ok()
            .filter(|v| !v.trim().is_empty())
    }

    pub fn resolved_aws_session_token(&self) -> Option<String> {
        if !self.remote.aws_session_token.trim().is_empty() {
            return Some(self.remote.aws_session_token.trim().to_string());
        }
        std::env::var("AWS_SESSION_TOKEN")
            .ok()
            .filter(|v| !v.trim().is_empty())
    }

    pub fn has_stored_aws_session_token(&self) -> bool {
        !self.remote.aws_session_token.trim().is_empty()
            || self.remote.aws_session_token_credential_id.is_some()
    }

    fn access_key_requires_session_token(&self) -> bool {
        if self.remote.api_key.trim().starts_with("ASIA") {
            return true;
        }
        self.resolved_api_key()
            .is_some_and(|key| key.starts_with("ASIA"))
    }

    pub fn has_stored_api_key(&self) -> bool {
        !self.remote.api_key.trim().is_empty() || self.remote.api_key_credential_id.is_some()
    }

    pub fn has_stored_aws_secret(&self) -> bool {
        !self.remote.aws_secret_access_key.trim().is_empty()
            || self.remote.aws_secret_access_key_credential_id.is_some()
    }

    /// Validate remote credentials for connectivity tests (no env fallback unless explicitly allowed).
    pub fn validate_remote_for_test(&self, allow_env_fallback: bool) -> Result<(), String> {
        if self.remote.model.trim().is_empty() {
            return Err("model name is required".into());
        }

        match self.remote.provider {
            RemoteProvider::Bedrock => {
                if !self.has_stored_api_key() {
                    return Err("access key id is required".into());
                }
                if !self.has_stored_aws_secret()
                    && !(allow_env_fallback && self.resolved_aws_secret_access_key().is_some())
                {
                    return Err("secret access key is required".into());
                }
                if self
                    .remote
                    .aws_region
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .is_none()
                {
                    return Err("region is required".into());
                }
                if self.access_key_requires_session_token()
                    && !self.has_stored_aws_session_token()
                    && !(allow_env_fallback && self.resolved_aws_session_token().is_some())
                {
                    return Err(
                        "session token is required for temporary AWS credentials (ASIA access keys)"
                            .into(),
                    );
                }
            }
            RemoteProvider::Azure => {
                if self
                    .remote
                    .base_url
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .is_none()
                {
                    return Err("endpoint url is required".into());
                }
                if !self.has_stored_api_key()
                    && !(allow_env_fallback && self.resolved_api_key().is_some())
                {
                    return Err("api key is required".into());
                }
            }
            _ => {
                if !self.has_stored_api_key()
                    && !(allow_env_fallback && self.resolved_api_key().is_some())
                {
                    return Err("api key is required".into());
                }
            }
        }

        Ok(())
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
