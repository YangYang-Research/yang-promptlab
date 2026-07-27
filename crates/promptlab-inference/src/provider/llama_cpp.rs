use std::sync::Arc;

use async_trait::async_trait;
use promptlab_models::runtime::InferenceRuntime;
use promptlab_models::types::InferenceRequest;
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
        // Leaf choke point for local AI Runtime completions (agents / gateway / judge).
        crate::traffic::record_sent();
        let runtime = self.runtime.lock().await;
        let response = runtime
            .complete(InferenceRequest {
                system: system.map(str::to_string),
                prompt: prompt.to_string(),
                max_tokens,
                temperature,
            })
            .await;
        match response {
            Ok(response) => {
                crate::traffic::record_received();
                let input = crate::token_usage::estimate_tokens(system.unwrap_or(""))
                    .saturating_add(crate::token_usage::estimate_tokens(prompt));
                let output = if response.tokens_predicted > 0 {
                    response.tokens_predicted as u64
                } else {
                    crate::token_usage::estimate_tokens(&response.text)
                };
                crate::token_usage::record_completion(input, output);
                Ok(response.text)
            }
            Err(err) => Err(err.into()),
        }
    }

    async fn health(&self) -> InferenceResult<bool> {
        let runtime = self.runtime.lock().await;
        runtime.health().await.map_err(InferenceError::from)
    }
}
