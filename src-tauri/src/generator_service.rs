//! Bridge judge local LLM backend into the payload generator.

use std::sync::Arc;

use aisec_generator::GeneratorLlm;
use aisec_judge::providers::LlmBackend;
use async_trait::async_trait;

pub struct JudgeGeneratorLlm {
    backend: Arc<dyn LlmBackend>,
}

impl JudgeGeneratorLlm {
    pub fn new(backend: Arc<dyn LlmBackend>) -> Self {
        Self { backend }
    }
}

#[async_trait]
impl GeneratorLlm for JudgeGeneratorLlm {
    async fn complete(&self, prompt: &str) -> aisec_generator::GeneratorResult<String> {
        self.backend
            .complete(prompt, 1536, 0.2)
            .await
            .map_err(|e| aisec_generator::GeneratorError::Llm(e.to_string()))
    }
}
