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
mod regex;
mod rule;

pub use llm::{LlmEvaluator, LlmResponseParser};
pub use regex::{RegexEvaluator, RegexRule};
pub use rule::{RuleBasedEvaluator, RuleSet, SignalRule};
