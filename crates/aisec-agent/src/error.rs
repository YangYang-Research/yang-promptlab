use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("analyze endpoint failed: {0}")]
    AnalyzeEndpoint(String),
    #[error("attack plan failed: {0}")]
    AttackPlan(String),
    #[error("generate prompt failed: {0}")]
    GeneratePrompt(String),
    #[error("supervisor: {0}")]
    Supervisor(String),
}

pub type AgentResult<T> = Result<T, AgentError>;
