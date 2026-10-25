use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    UnknownAgent(String),
    UnknownPrompt(String),
    UnknownTool(String),
    PromptNotOwned { agent: &'static str, prompt: String },
    ToolNotOwned { agent: &'static str, tool: String },
    NoPrompt(&'static str),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownAgent(id) => write!(f, "unknown agent id: {id}"),
            Self::UnknownPrompt(id) => write!(f, "unknown prompt id: {id}"),
            Self::UnknownTool(id) => write!(f, "unknown tool id: {id}"),
            Self::PromptNotOwned { agent, prompt } => {
                write!(f, "agent {agent} does not own prompt {prompt}")
            }
            Self::ToolNotOwned { agent, tool } => {
                write!(f, "agent {agent} does not own tool {tool}")
            }
            Self::NoPrompt(agent) => write!(f, "agent {agent} has no prompt"),
        }
    }
}

impl std::error::Error for RegistryError {}
