use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlannerError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("planning failed: {0}")]
    Planning(String),
    #[error("llm planning failed: {0}")]
    Llm(String),
}

pub type PlannerResult<T> = Result<T, PlannerError>;
