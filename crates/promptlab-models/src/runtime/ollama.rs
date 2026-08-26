use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde_json::json;

use crate::error::{ModelError, ModelResult};
use crate::runtime::InferenceRuntime;
use crate::types::{
    ChatMessage, ChatRequest, ChatResponse, EmbeddingRequest, EmbeddingResponse,
    InferenceRequest, InferenceResponse, RuntimeState,
};

/// Ollama HTTP API runtime (`POST /api/generate`).
#[derive(Debug, Clone)]
pub struct OllamaConfig {
    pub base_url: String,
    pub model: String,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:11434".into(),
            model: "llama3".into(),
        }
    }
}

pub struct OllamaRuntime {
    config: OllamaConfig,
    client: reqwest::Client,
    state: Arc<AtomicU32>,
}

impl OllamaRuntime {
    pub fn new(config: OllamaConfig) -> Self {
        let client = promptlab_core::default_http_client().unwrap_or_else(|_| reqwest::Client::new());
        Self {
            config,
            client,
            state: Arc::new(AtomicU32::new(RuntimeState::Ready as u32)),
        }
    }

    pub fn config(&self) -> &OllamaConfig {
        &self.config
    }

    pub async fn check_connectivity(&self) -> ModelResult<bool> {
        let url = format!("{}/api/tags", self.config.base_url.trim_end_matches('/'));
        let response = self
            .client
            .get(url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| ModelError::runtime(format!("ollama unreachable: {e}")))?;
        Ok(response.status().is_success())
    }

    pub async fn chat(&self, request: ChatRequest) -> ModelResult<ChatResponse> {
        let started = Instant::now();
        let url = format!("{}/api/chat", self.config.base_url.trim_end_matches('/'));
        let messages: Vec<_> = request
            .messages
            .iter()
            .map(|m| json!({ "role": m.role, "content": m.content }))
            .collect();
        let body = json!({
            "model": self.config.model,
            "messages": messages,
            "stream": false,
            "options": {
                "temperature": request.temperature,
                "num_predict": request.max_tokens,
            }
        });

        let response = self
            .client
            .post(url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| ModelError::runtime(format!("ollama chat failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(ModelError::runtime(format!(
                "ollama chat returned {status}: {text}"
            )));
        }

        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ModelError::runtime(format!("ollama chat parse failed: {e}")))?;

        let message = value
            .get("message")
            .map(|m| ChatMessage {
                role: m
                    .get("role")
                    .and_then(|v| v.as_str())
                    .unwrap_or("assistant")
                    .into(),
                content: m
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .into(),
            })
            .unwrap_or(ChatMessage {
                role: "assistant".into(),
                content: String::new(),
            });

        Ok(ChatResponse {
            message,
            tokens_predicted: value
                .get("eval_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            duration_ms: started.elapsed().as_millis() as u64,
        })
    }

    pub async fn embeddings(&self, request: EmbeddingRequest) -> ModelResult<EmbeddingResponse> {
        let started = Instant::now();
        let url = format!(
            "{}/api/embeddings",
            self.config.base_url.trim_end_matches('/')
        );
        let body = json!({
            "model": self.config.model,
            "prompt": request.input,
        });

        let response = self
            .client
            .post(url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await
            .map_err(|e| ModelError::runtime(format!("ollama embeddings failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(ModelError::runtime(format!(
                "ollama embeddings returned {status}: {text}"
            )));
        }

        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ModelError::runtime(format!("ollama embeddings parse failed: {e}")))?;

        let vector: Vec<f32> = value
            .get("embedding")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|n| n.as_f64().map(|f| f as f32))
                    .collect()
            })
            .unwrap_or_default();

        Ok(EmbeddingResponse {
            dimensions: vector.len(),
            vector,
            duration_ms: started.elapsed().as_millis() as u64,
        })
    }

    pub async fn pull_model(&self) -> ModelResult<()> {
        let url = format!("{}/api/pull", self.config.base_url.trim_end_matches('/'));
        let body = json!({
            "name": self.config.model,
            "stream": false,
        });

        let response = self
            .client
            .post(url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(600))
            .send()
            .await
            .map_err(|e| ModelError::download(format!("ollama pull failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(ModelError::download(format!(
                "ollama pull returned {status}: {text}"
            )));
        }
        Ok(())
    }
}

#[async_trait]
impl InferenceRuntime for OllamaRuntime {
    fn state(&self) -> RuntimeState {
        match self.state.load(Ordering::SeqCst) {
            0 => RuntimeState::Unloaded,
            1 => RuntimeState::Loading,
            2 => RuntimeState::Ready,
            _ => RuntimeState::Error,
        }
    }

    async fn complete(&self, request: InferenceRequest) -> ModelResult<InferenceResponse> {
        let url = format!(
            "{}/api/generate",
            self.config.base_url.trim_end_matches('/')
        );
        let body = json!({
            "model": self.config.model,
            "prompt": request.prompt,
            "stream": false,
            "options": {
                "temperature": request.temperature,
                "num_predict": request.max_tokens,
            }
        });

        let response = self
            .client
            .post(url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| ModelError::runtime(format!("ollama request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(ModelError::runtime(format!(
                "ollama returned {status}: {text}"
            )));
        }

        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ModelError::runtime(format!("ollama response parse failed: {e}")))?;

        let text = value
            .get("response")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(InferenceResponse {
            text,
            tokens_predicted: value
                .get("eval_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            duration_ms: 0,
        })
    }

    async fn health(&self) -> ModelResult<bool> {
        self.check_connectivity().await
    }
}
