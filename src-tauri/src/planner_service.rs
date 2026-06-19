//! Bridge judge local LLM backend into the attack planner.

use std::sync::Arc;

use aisec_judge::providers::LlmBackend;
use aisec_planner::PlannerLlm;
use async_trait::async_trait;

pub struct JudgePlannerLlm {
    backend: Arc<dyn LlmBackend>,
}

impl JudgePlannerLlm {
    pub fn new(backend: Arc<dyn LlmBackend>) -> Self {
        Self { backend }
    }
}

#[async_trait]
impl PlannerLlm for JudgePlannerLlm {
    async fn complete(&self, prompt: &str) -> aisec_planner::PlannerResult<String> {
        self.backend
            .complete(prompt, 1024, 0.15)
            .await
            .map_err(|e| aisec_planner::PlannerError::Llm(e.to_string()))
    }
}
