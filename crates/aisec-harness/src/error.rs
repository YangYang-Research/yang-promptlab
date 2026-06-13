use thiserror::Error;

#[derive(Debug, Error)]
pub enum HarnessError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("unsupported target surface: {0}")]
    UnsupportedSurface(String),
    #[error("authentication error: {0}")]
    Auth(String),
    #[error("normalization error: {0}")]
    Normalization(String),
    #[error("harness not found: {0}")]
    NotFound(String),
}

pub type HarnessResult<T> = Result<T, HarnessError>;

impl HarnessError {
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config(message.into())
    }

    pub fn transport(message: impl Into<String>) -> Self {
        Self::Transport(message.into())
    }
}

impl From<reqwest::Error> for HarnessError {
    fn from(value: reqwest::Error) -> Self {
        Self::Transport(value.to_string())
    }
}

impl From<serde_json::Error> for HarnessError {
    fn from(value: serde_json::Error) -> Self {
        Self::Normalization(value.to_string())
    }
}
