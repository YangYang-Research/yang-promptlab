use thiserror::Error;

use promptlab_core::PromptLabError;

#[derive(Debug, Error)]
pub enum PayloadError {
    #[error("payload not found: {0}")]
    NotFound(String),

    #[error("invalid payload data: {0}")]
    InvalidData(String),

    #[error("mutation failed: {0}")]
    Mutation(String),

    #[error("pipeline error: {0}")]
    Pipeline(String),

    #[error(transparent)]
    Core(#[from] PromptLabError),
}

pub type PayloadResult<T> = Result<T, PayloadError>;

impl PayloadError {
    pub fn not_found(id: impl Into<String>) -> Self {
        Self::NotFound(id.into())
    }

    pub fn invalid_data(msg: impl Into<String>) -> Self {
        Self::InvalidData(msg.into())
    }

    pub fn mutation(msg: impl Into<String>) -> Self {
        Self::Mutation(msg.into())
    }
}
