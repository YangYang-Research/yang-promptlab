use thiserror::Error;

use aisec_core::AisecError;

/// Attack framework errors.
#[derive(Debug, Error)]
pub enum AttackError {
    #[error("attack not found: {0}")]
    NotFound(String),

    #[error("invalid attack state: {0}")]
    InvalidState(String),

    #[error("payload error: {0}")]
    Payload(String),

    #[error("transport error: {0}")]
    Transport(String),

    #[error("evaluation error: {0}")]
    Evaluation(String),

    #[error("orchestration cancelled")]
    Cancelled,

    #[error("budget exhausted: {0}")]
    BudgetExhausted(String),

    #[error(transparent)]
    Core(#[from] AisecError),
}

pub type AttackResult<T> = Result<T, AttackError>;

impl AttackError {
    pub fn invalid_state(msg: impl Into<String>) -> Self {
        Self::InvalidState(msg.into())
    }

    pub fn payload(msg: impl Into<String>) -> Self {
        Self::Payload(msg.into())
    }

    pub fn transport(msg: impl Into<String>) -> Self {
        Self::Transport(msg.into())
    }
}
