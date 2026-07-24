use thiserror::Error;

use promptlab_core::PromptLabError;
use promptlab_models::ModelError;

#[derive(Debug, Error)]
pub enum JudgeError {
    #[error("evaluation error: {0}")]
    Evaluation(String),

    #[error("model role not configured: {0}")]
    RoleNotConfigured(String),

    #[error("invalid configuration: {0}")]
    Config(String),

    #[error("consensus failed: {0}")]
    Consensus(String),

    #[error(transparent)]
    Model(#[from] ModelError),

    #[error(transparent)]
    Core(#[from] PromptLabError),
}

pub type JudgeResult<T> = Result<T, JudgeError>;

impl JudgeError {
    pub fn evaluation(msg: impl Into<String>) -> Self {
        Self::Evaluation(msg.into())
    }

    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }
}
