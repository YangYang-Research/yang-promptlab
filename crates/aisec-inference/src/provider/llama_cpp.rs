use std::sync::Arc;

use async_trait::async_trait;
use aisec_models::runtime::InferenceRuntime;
use aisec_models::types::InferenceRequest;
use tokio::sync::Mutex;

use super::ProviderAdapter;
use crate::capabilities::ModelCapabilities;
use crate::config::InferenceProvider;
use crate::error::{InferenceError, InferenceResult};

pub struct LlamaCppAdapter {
    provider: InferenceProvider,
    model: String,
    runtime: Arc<Mutex<dyn InferenceRuntime>>,
}

impl LlamaCppAdapter {
    pub fn new(
        provider: InferenceProvider,
        model: impl Into<String>,
        runtime: Arc<Mutex<dyn InferenceRuntime>>,
    ) -> Self {
        Self {
            provider,
            model: model.into(),
            runtime,
        }
    }
}

#[async_trait]
impl ProviderAdapter for LlamaCppAdapter {
    fn provider_id(&self) -> &str {
        self.provider.as_str()
    }

    fn model_id(&self) -> &str {
        &self.model
    }

    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities::from_local_chat()
    }

    async fn complete(
        &self,
        system: Option<&str>,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> InferenceResult<String> {
        let full_prompt = match system {
            Some(sys) if !sys.trim().is_empty() => format!("{sys}\n\n{prompt}"),
            _ => prompt.to_string(),
        };
        let runtime = self.runtime.lock().await;
        let response = runtime
            .complete(InferenceRequest {
                prompt: full_prompt,
                max_tokens,
                temperature,
            })
            .await?;
        Ok(response.text)
    }

    async fn health(&self) -> InferenceResult<bool> {
        let runtime = self.runtime.lock().await;
        runtime.health().await.map_err(InferenceError::from)
    }
}
