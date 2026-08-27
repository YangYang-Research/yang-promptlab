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
    #[error("cancelled")]
    Cancelled,
    #[error("denied by interceptor: {0}")]
    Denied(String),
    #[error("rate limited")]
    RateLimited { retry_after_ms: Option<u64> },
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("empty completion")]
    Empty,
    #[error("HTTP {status}: {message}")]
    Http { status: u16, message: String },
}

pub type HarnessResult<T> = Result<T, HarnessError>;

impl HarnessError {
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config(message.into())
    }

    pub fn transport(message: impl Into<String>) -> Self {
        Self::Transport(message.into())
    }

    pub fn auth(message: impl Into<String>) -> Self {
        Self::Auth(message.into())
    }

    pub fn error_class(&self) -> &'static str {
        match self {
            Self::Auth(_) => "auth",
            Self::Cancelled => "cancelled",
            Self::RateLimited { .. } => "rate_limit",
            Self::Timeout(_) => "timeout",
            Self::Empty => "empty",
            Self::Denied(_) => "denied",
            Self::Config(_) | Self::UnsupportedSurface(_) | Self::NotFound(_) => "config",
            Self::Normalization(_) | Self::Http { .. } => "http",
            Self::Transport(_) => "transport",
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimited { .. } | Self::Timeout(_) | Self::Empty | Self::Transport(_)
        )
    }
}

impl From<reqwest::Error> for HarnessError {
    fn from(value: reqwest::Error) -> Self {
        if value.is_timeout() {
            Self::Timeout(value.to_string())
        } else {
            Self::Transport(value.to_string())
        }
    }
}

impl From<serde_json::Error> for HarnessError {
    fn from(value: serde_json::Error) -> Self {
        Self::Normalization(value.to_string())
    }
}
