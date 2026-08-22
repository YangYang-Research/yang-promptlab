use thiserror::Error;

#[derive(Debug, Error)]
pub enum InferenceError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("runtime not ready: {0}")]
    NotReady(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("capability not supported: {0}")]
    Unsupported(String),
    #[error("prompt error: {0}")]
    Prompt(String),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("internal error: {0}")]
    Internal(String),
}

pub type InferenceResult<T> = Result<T, InferenceError>;

impl From<promptlab_models::error::ModelError> for InferenceError {
    fn from(value: promptlab_models::error::ModelError) -> Self {
        Self::Provider(value.to_string())
    }
}

impl From<promptlab_harness::HarnessError> for InferenceError {
    fn from(value: promptlab_harness::HarnessError) -> Self {
        Self::Provider(value.to_string())
    }
}

impl From<promptlab_runtime::error::RuntimeError> for InferenceError {
    fn from(value: promptlab_runtime::error::RuntimeError) -> Self {
        Self::Internal(value.to_string())
    }
}
