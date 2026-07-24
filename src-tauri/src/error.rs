use serde::Serialize;

use promptlab_core::{PromptLabError, ErrorCode};

/// IPC-facing error envelope returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl CommandError {
    pub fn from_promptlab(error: PromptLabError) -> Self {
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

impl From<PromptLabError> for CommandError {
    fn from(value: PromptLabError) -> Self {
        Self::from_promptlab(value)
    }
}

impl From<promptlab_harness::HarnessError> for CommandError {
    fn from(value: promptlab_harness::HarnessError) -> Self {
        Self {
            code: ErrorCode::Internal.as_str().to_string(),
            message: value.to_string(),
        }
    }
}

impl From<promptlab_runtime::RuntimeError> for CommandError {
    fn from(value: promptlab_runtime::RuntimeError) -> Self {
        Self {
            code: ErrorCode::Internal.as_str().to_string(),
            message: value.to_string(),
        }
    }
}

impl From<promptlab_plugin_host::PluginError> for CommandError {
    fn from(value: promptlab_plugin_host::PluginError) -> Self {
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
    fn maps_promptlab_error_to_command_error() {
        let cmd_err = CommandError::from(PromptLabError::not_found("project"));
        assert_eq!(cmd_err.code, "NOT_FOUND");
    }
}
