use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable error codes surfaced to IPC clients and logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    Internal,
    Config,
    Io,
    NotFound,
    InvalidInput,
    Unauthorized,
    Plugin,
    Storage,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Internal => "INTERNAL",
            Self::Config => "CONFIG",
            Self::Io => "IO",
            Self::NotFound => "NOT_FOUND",
            Self::InvalidInput => "INVALID_INPUT",
            Self::Unauthorized => "UNAUTHORIZED",
            Self::Plugin => "PLUGIN",
            Self::Storage => "STORAGE",
        }
    }
}

/// Application-wide error type for Rust services.
#[derive(Debug, Error)]
pub enum PromptLabError {
    #[error("[{code}] {message}")]
    Tagged {
        code: ErrorCode,
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("configuration error: {0}")]
    Config(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("resource not found: {0}")]
    NotFound(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type PromptLabResult<T> = Result<T, PromptLabError>;

impl PromptLabError {
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Tagged {
            code: ErrorCode::Internal,
            message: message.into(),
            source: None,
        }
    }

    pub fn config(message: impl Into<String>) -> Self {
        Self::Config(message.into())
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Tagged { code, .. } => *code,
            Self::Config(_) => ErrorCode::Config,
            Self::InvalidInput(_) => ErrorCode::InvalidInput,
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::Io(_) => ErrorCode::Io,
        }
    }

    pub fn client_message(&self) -> String {
        match self {
            Self::Tagged { message, .. } => message.clone(),
            Self::Config(msg) => msg.clone(),
            Self::InvalidInput(msg) => msg.clone(),
            Self::NotFound(msg) => msg.clone(),
            Self::Io(err) => err.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(PromptLabError::config("missing key").code(), ErrorCode::Config);
        assert_eq!(
            PromptLabError::invalid_input("bad uuid").code(),
            ErrorCode::InvalidInput
        );
    }
}
