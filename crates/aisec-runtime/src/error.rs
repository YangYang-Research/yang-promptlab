use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("process error: {0}")]
    Process(String),
    #[error("registry error: {0}")]
    Registry(String),
    #[error("model error: {0}")]
    Model(String),
    #[error("provider unavailable")]
    Unavailable,
}

pub type RuntimeResult<T> = Result<T, RuntimeError>;

impl From<aisec_models::error::ModelError> for RuntimeError {
    fn from(value: aisec_models::error::ModelError) -> Self {
        Self::Model(value.to_string())
    }
}

impl From<aisec_core::AisecError> for RuntimeError {
    fn from(value: aisec_core::AisecError) -> Self {
        Self::Config(value.to_string())
    }
}
