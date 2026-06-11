use async_trait::async_trait;
use aisec_models::runtime::InferenceRuntime;
use aisec_models::types::{InferenceRequest, InferenceResponse, RuntimeState};

/// Mock llama.cpp runtime returning fixed JSON for judge tests.
pub struct JsonMockRuntime {
    response_json: String,
    ready: bool,
}

impl JsonMockRuntime {
    pub fn new(json: impl Into<String>) -> Self {
        Self {
            response_json: json.into(),
            ready: false,
        }
    }

    pub fn judge_vulnerable(confidence: f32) -> Self {
        Self::new(format!(
            r#"{{"vulnerable": true, "confidence": {confidence}, "severity": "high", "rationale": "mock judge", "indicators": ["mock"]}}"#
        ))
    }

    pub fn classifier(category: &str) -> Self {
        Self::new(format!(
            r#"{{"vulnerable": true, "confidence": 0.8, "category": "{category}", "severity": "medium", "rationale": "mock classifier", "indicators": []}}"#
        ))
    }
}

#[async_trait]
impl InferenceRuntime for JsonMockRuntime {
    fn state(&self) -> RuntimeState {
        if self.ready {
            RuntimeState::Ready
        } else {
            RuntimeState::Unloaded
        }
    }

    async fn load_model(&mut self, _model_path: &std::path::Path) -> aisec_models::ModelResult<()> {
        self.ready = true;
        Ok(())
    }

    async fn unload(&mut self) -> aisec_models::ModelResult<()> {
        self.ready = false;
        Ok(())
    }

    async fn complete(
        &self,
        _request: InferenceRequest,
    ) -> aisec_models::ModelResult<InferenceResponse> {
        if !self.ready {
            // Auto-ready for tests without explicit load
        }
        Ok(InferenceResponse {
            text: self.response_json.clone(),
            tokens_predicted: 32,
            duration_ms: 1,
        })
    }

    async fn health(&self) -> aisec_models::ModelResult<bool> {
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn returns_json() {
        let runtime = JsonMockRuntime::judge_vulnerable(0.95);
        let resp = runtime
            .complete(InferenceRequest {
                prompt: "test".into(),
                max_tokens: 64,
                temperature: 0.0,
            })
            .await
            .unwrap();
        assert!(resp.text.contains("vulnerable"));
    }
}
