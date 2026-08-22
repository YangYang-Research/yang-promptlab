use std::sync::Arc;

use async_trait::async_trait;
use promptlab_harness::{AttackRequest, Harness, HarnessResult, NormalizedResponse};
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
impl Harness for LlamaCppAdapter {
    fn id(&self) -> &'static str {
        "llama"
    }

    async fn execute(&self, request: AttackRequest) -> HarnessResult<NormalizedResponse> {
        request.cancel.check()?;
        let (system, prompt) = request.system_and_user_prompt();
        let runtime = self.runtime.lock().await;
        let response = runtime
            .complete(InferenceRequest {
                system,
                prompt,
                max_tokens: request.max_tokens.unwrap_or(1024),
                temperature: request.temperature.unwrap_or(0.0),
            })
            .await
            .map_err(|err| promptlab_harness::HarnessError::transport(err.to_string()))?;
        let mut normalized = NormalizedResponse::from_chat(response.text, self.id());
        if response.tokens_predicted > 0 {
            normalized.usage_output_tokens = Some(response.tokens_predicted as u64);
        }
        if normalized.content.trim().is_empty() {
            normalized.error_class = Some("empty".into());
        }
        Ok(normalized)
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
        let mut request = AttackRequest::from_payload("local://llama", prompt);
        request.purpose = promptlab_harness::HarnessPurpose::assistant();
        request.system = system.map(str::to_string);
        request.max_tokens = Some(max_tokens);
        request.temperature = Some(temperature);
        let response = Harness::execute(self, request)
            .await
            .map_err(|err| InferenceError::Provider(err.to_string()))?;
        if response.content.trim().is_empty() {
            return Err(InferenceError::Provider("empty local completion".into()));
        }
        Ok(response.content)
    }

    async fn health(&self) -> InferenceResult<bool> {
        let runtime = self.runtime.lock().await;
        runtime.health().await.map_err(InferenceError::from)
    }
}
