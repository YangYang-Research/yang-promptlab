use thiserror::Error;

use aisec_core::AisecError;

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("model not found: {0}")]
    NotFound(String),

    #[error("invalid model: {0}")]
    Invalid(String),

    #[error("download error: {0}")]
    Download(String),

    #[error("verification failed: {0}")]
    Verification(String),

    #[error("runtime error: {0}")]
    Runtime(String),

    #[error("hardware detection error: {0}")]
    Hardware(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Core(#[from] AisecError),
}

pub type ModelResult<T> = Result<T, ModelError>;

impl ModelError {
    pub fn not_found(id: impl Into<String>) -> Self {
        Self::NotFound(id.into())
    }

    pub fn invalid(msg: impl Into<String>) -> Self {
        Self::Invalid(msg.into())
    }

    pub fn download(msg: impl Into<String>) -> Self {
        Self::Download(msg.into())
    }

    pub fn verification(msg: impl Into<String>) -> Self {
        Self::Verification(msg.into())
    }

    pub fn runtime(msg: impl Into<String>) -> Self {
        Self::Runtime(msg.into())
    }
}
