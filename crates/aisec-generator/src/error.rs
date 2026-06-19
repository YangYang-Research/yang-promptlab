use thiserror::Error;

#[derive(Debug, Error)]
pub enum GeneratorError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("payload library: {0}")]
    Payload(#[from] aisec_payload::PayloadError),
    #[error("LLM generation failed: {0}")]
    Llm(String),
    #[error("serialization: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type GeneratorResult<T> = Result<T, GeneratorError>;
