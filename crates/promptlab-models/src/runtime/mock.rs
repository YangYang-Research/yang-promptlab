use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use crate::error::ModelResult;
use crate::runtime::InferenceRuntime;
use crate::types::{InferenceRequest, InferenceResponse, RuntimeState};

/// Mock inference runtime for tests.
pub struct MockInferenceRuntime {
    state: Arc<AtomicU32>,
    response_text: String,
}

impl MockInferenceRuntime {
    pub fn new(response_text: impl Into<String>) -> Self {
        Self {
            state: Arc::new(AtomicU32::new(RuntimeState::Ready as u32)),
            response_text: response_text.into(),
        }
    }
}

#[async_trait]
impl InferenceRuntime for MockInferenceRuntime {
    fn state(&self) -> RuntimeState {
        match self.state.load(Ordering::SeqCst) {
            x if x == RuntimeState::Unloaded as u32 => RuntimeState::Unloaded,
            x if x == RuntimeState::Loading as u32 => RuntimeState::Loading,
            x if x == RuntimeState::Ready as u32 => RuntimeState::Ready,
            _ => RuntimeState::Error,
        }
    }

    async fn complete(&self, request: InferenceRequest) -> ModelResult<InferenceResponse> {
        if self.state() != RuntimeState::Ready {
            return Err(crate::error::ModelError::runtime("not ready"));
        }
        Ok(InferenceResponse {
            text: format!("{} [mock: {}]", self.response_text, request.prompt),
            tokens_predicted: 16,
            duration_ms: 1,
        })
    }

    async fn health(&self) -> ModelResult<bool> {
        Ok(self.state() == RuntimeState::Ready)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_complete() {
        let runtime = MockInferenceRuntime::new("ok");
        let resp = runtime
            .complete(InferenceRequest {
                system: None,
                prompt: "hello".into(),
                max_tokens: 32,
                temperature: 0.7,
            })
            .await
            .unwrap();
        assert!(resp.text.contains("ok"));
    }
}
