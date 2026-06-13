use thiserror::Error;

#[derive(Debug, Error)]
pub enum BrowserError {
    #[error("storage error: {0}")]
    Storage(String),
    #[error("session not found: {0}")]
    NotFound(String),
    #[error("session expired")]
    Expired,
    #[error("validation failed: {0}")]
    Validation(String),
}

pub type BrowserResult<T> = Result<T, BrowserError>;

impl From<aisec_core::AisecError> for BrowserError {
    fn from(value: aisec_core::AisecError) -> Self {
        Self::Storage(value.to_string())
    }
}
