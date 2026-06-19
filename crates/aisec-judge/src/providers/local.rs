use std::sync::Arc;

use async_trait::async_trait;
use aisec_models::runtime::InferenceRuntime;
use aisec_models::types::InferenceRequest;
use tokio::sync::Mutex;

use super::LlmBackend;
use crate::error::{JudgeError, JudgeResult};

pub struct LocalLlmBackend {
    label: String,
    model: String,
    runtime: Arc<Mutex<dyn InferenceRuntime>>,
}

impl LocalLlmBackend {
    pub fn new(label: impl Into<String>, model: impl Into<String>, runtime: Arc<Mutex<dyn InferenceRuntime>>) -> Self {
        Self {
            label: label.into(),
            model: model.into(),
            runtime,
        }
    }
}

#[async_trait]
impl LlmBackend for LocalLlmBackend {
    fn provider_label(&self) -> &str {
        &self.label
    }

    fn model_label(&self) -> &str {
        &self.model
    }

    async fn complete(&self, prompt: &str, max_tokens: u32, temperature: f32) -> JudgeResult<String> {
        let runtime = self.runtime.lock().await;
        let response = runtime
            .complete(InferenceRequest {
                prompt: prompt.to_string(),
                max_tokens,
                temperature,
            })
            .await
            .map_err(|e| JudgeError::evaluation(e.to_string()))?;
        Ok(response.text)
    }

    async fn health_check(&self) -> JudgeResult<bool> {
        let runtime = self.runtime.lock().await;
        runtime
            .health()
            .await
            .map_err(|e| JudgeError::evaluation(e.to_string()))
    }
}
