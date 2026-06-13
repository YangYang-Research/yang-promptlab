use serde::Serialize;

use aisec_core::{AisecError, ErrorCode};

/// IPC-facing error envelope returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl CommandError {
    pub fn from_aisec(error: AisecError) -> Self {
        Self {
            code: error.code().as_str().to_string(),
            message: error.client_message(),
        }
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::InvalidInput.as_str().to_string(),
            message: message.into(),
        }
    }
}

impl From<AisecError> for CommandError {
    fn from(value: AisecError) -> Self {
        Self::from_aisec(value)
    }
}

impl From<aisec_harness::HarnessError> for CommandError {
    fn from(value: aisec_harness::HarnessError) -> Self {
        Self {
            code: ErrorCode::Internal.as_str().to_string(),
            message: value.to_string(),
        }
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for CommandError {}

pub type CommandResult<T> = Result<T, CommandError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_aisec_error_to_command_error() {
        let cmd_err = CommandError::from(AisecError::not_found("project"));
        assert_eq!(cmd_err.code, "NOT_FOUND");
    }
}
