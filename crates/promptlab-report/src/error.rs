use thiserror::Error;

use aisec_core::AisecError;

#[derive(Debug, Error)]
pub enum ReportError {
    #[error("render error: {0}")]
    Render(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Core(#[from] AisecError),
}

pub type ReportResult<T> = Result<T, ReportError>;

impl ReportError {
    pub fn render(msg: impl Into<String>) -> Self {
        Self::Render(msg.into())
    }

    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::InvalidInput(msg.into())
    }
}
