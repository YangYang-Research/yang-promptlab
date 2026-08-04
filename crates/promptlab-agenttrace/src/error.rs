use thiserror::Error;

pub type Result<T> = std::result::Result<T, AgentTraceError>;

#[derive(Debug, Error)]
pub enum AgentTraceError {
    #[error("agenttrace io: {0}")]
    Io(#[from] std::io::Error),
    #[error("agenttrace sql: {0}")]
    Sql(#[from] sqlx::Error),
    #[error("agenttrace: {0}")]
    Message(String),
}

impl AgentTraceError {
    pub fn msg(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }
}
