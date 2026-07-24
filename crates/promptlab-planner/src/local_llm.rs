use async_trait::async_trait;

use crate::error::PlannerResult;

/// LLM completion bridge for wizard attack-plan refinement.
#[async_trait]
pub trait PlannerLlm: Send + Sync {
    async fn complete(&self, prompt: &str) -> PlannerResult<String>;
}
