use thiserror::Error;

use promptlab_core::PromptLabError;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("plugin not found: {0}")]
    NotFound(String),

    #[error("invalid manifest: {0}")]
    InvalidManifest(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("lifecycle error: {0}")]
    Lifecycle(String),

    #[error("sandbox error: {0}")]
    Sandbox(String),

    #[error("version incompatible: {0}")]
    VersionIncompatible(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Core(#[from] PromptLabError),
}

pub type PluginResult<T> = Result<T, PluginError>;

impl PluginError {
    pub fn not_found(id: impl Into<String>) -> Self {
        Self::NotFound(id.into())
    }

    pub fn permission_denied(msg: impl Into<String>) -> Self {
        Self::PermissionDenied(msg.into())
    }

    pub fn sandbox(msg: impl Into<String>) -> Self {
        Self::Sandbox(msg.into())
    }
}
