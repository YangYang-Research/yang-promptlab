use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("cancelled")]
    Cancelled,
    #[error("planner: {0}")]
    Planner(#[from] aisec_planner::PlannerError),
    #[error("generator: {0}")]
    Generator(#[from] aisec_generator::GeneratorError),
    #[error("attack: {0}")]
    Attack(String),
    #[error("judge: {0}")]
    Judge(String),
    #[error("host: {0}")]
    Host(String),
}

pub type AgentResult<T> = Result<T, AgentError>;
