use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("model not installed: {0}")]
    ModelNotInstalled(String),

    #[error("model not loaded")]
    ModelNotLoaded,

    #[error("model load failed: {0}")]
    ModelLoadFailed(String),

    #[error("backend unavailable: {0}")]
    BackendUnavailable(String),

    #[error("CUDA unavailable")]
    CudaUnavailable,

    #[error("Metal unavailable")]
    MetalUnavailable,

    #[error("out of memory")]
    OutOfMemory,

    #[error("invalid model: {0}")]
    InvalidModel(String),

    #[error("capability unavailable: {0}")]
    CapabilityUnavailable(String),

    #[error("inference cancelled")]
    InferenceCancelled,

    #[error("native runtime error: {0}")]
    NativeRuntimeError(String),

    #[error("registry error: {0}")]
    Registry(String),

    #[error("model error: {0}")]
    Model(String),

    #[error("provider unavailable")]
    Unavailable,
}

pub type RuntimeResult<T> = Result<T, RuntimeError>;

impl From<promptlab_models::error::ModelError> for RuntimeError {
    fn from(value: promptlab_models::error::ModelError) -> Self {
        let msg = value.to_string();
        if msg.contains("not found") {
            Self::ModelNotInstalled(msg)
        } else if msg.contains("invalid") {
            Self::InvalidModel(msg)
        } else if msg.contains("memory") || msg.contains("OOM") {
            Self::OutOfMemory
        } else {
            Self::NativeRuntimeError(msg)
        }
    }
}

impl From<promptlab_core::PromptLabError> for RuntimeError {
    fn from(value: promptlab_core::PromptLabError) -> Self {
        Self::Config(value.to_string())
    }
}
