use std::sync::Arc;

use async_trait::async_trait;
use promptlab_models::runtime::InferenceRuntime;
use promptlab_models::types::{InferenceRequest, InferenceResponse, RuntimeState};

use crate::error::RuntimeResult;
use crate::provider::ModelProvider;

/// Bridges [`ModelProvider`] to the [`InferenceRuntime`] contract used by the judge.
pub struct ModelProviderRuntime {
    provider: Arc<dyn ModelProvider>,
    model_id: String,
}

impl ModelProviderRuntime {
    pub fn new(provider: Arc<dyn ModelProvider>, model_id: impl Into<String>) -> Self {
        Self {
            provider,
            model_id: model_id.into(),
        }
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }
}

#[async_trait]
impl InferenceRuntime for ModelProviderRuntime {
    fn state(&self) -> RuntimeState {
        RuntimeState::Ready
    }

    async fn load_model(
        &mut self,
        _model_path: &std::path::Path,
    ) -> promptlab_models::error::ModelResult<()> {
        Ok(())
    }

    async fn unload(&mut self) -> promptlab_models::error::ModelResult<()> {
        Ok(())
    }

    async fn complete(
        &self,
        request: InferenceRequest,
    ) -> promptlab_models::error::ModelResult<InferenceResponse> {
        let response = self
            .provider
            .complete_for_model(&self.model_id, &request)
            .await
            .map_err(|err| promptlab_models::error::ModelError::runtime(err.to_string()))?;
        Ok(response)
    }

    async fn health(&self) -> promptlab_models::error::ModelResult<bool> {
        self.provider
            .health()
            .await
            .map(|health| health.healthy)
            .map_err(|err| promptlab_models::error::ModelError::runtime(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ModelProviderHealth;

    struct MockProvider;

    #[async_trait]
    impl ModelProvider for MockProvider {
        async fn list_models(&self) -> RuntimeResult<Vec<String>> {
            Ok(vec!["vault-model".into()])
        }

        async fn install_model(&self, _model_id: &str) -> RuntimeResult<()> {
            Ok(())
        }

        async fn remove_model(&self, _model_id: &str) -> RuntimeResult<()> {
            Ok(())
        }

        async fn run_inference(&self, _model_id: &str, _prompt: &str) -> RuntimeResult<String> {
            Ok("ok".into())
        }

        async fn complete_for_model(
            &self,
            _model_id: &str,
            request: &InferenceRequest,
        ) -> RuntimeResult<InferenceResponse> {
            Ok(InferenceResponse {
                text: format!("echo:{}", request.prompt),
                tokens_predicted: 1,
                duration_ms: 1,
            })
        }

        async fn health(&self) -> RuntimeResult<ModelProviderHealth> {
            Ok(ModelProviderHealth {
                healthy: true,
                message: "mock".into(),
            })
        }
    }

    #[tokio::test]
    async fn provider_runtime_completes_via_model_provider() {
        let provider: Arc<dyn ModelProvider> = Arc::new(MockProvider);
        let mut runtime = ModelProviderRuntime::new(provider, "vault-model");
        let response = runtime
            .complete(InferenceRequest {
                system: None,
                prompt: "probe".into(),
                max_tokens: 8,
                temperature: 0.0,
            })
            .await
            .expect("complete");
        assert!(response.text.contains("probe"));
    }
}
