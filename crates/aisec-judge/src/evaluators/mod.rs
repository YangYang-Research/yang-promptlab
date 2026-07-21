use async_trait::async_trait;

use crate::error::JudgeResult;
use crate::types::{EvaluatorResult, JudgeRequest};

/// Pluggable evaluation backend.
#[async_trait]
pub trait Evaluator: Send + Sync {
    fn id(&self) -> &str;
    async fn evaluate(&self, request: &JudgeRequest) -> JudgeResult<EvaluatorResult>;
}

mod llm;

pub use llm::{LlmEvaluator, LlmResponseParser};
