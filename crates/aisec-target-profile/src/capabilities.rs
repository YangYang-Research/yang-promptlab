use crate::types::{TargetCapabilities, TargetProvider};

pub fn default_capabilities_for_provider(provider: TargetProvider) -> TargetCapabilities {
    match provider {
        TargetProvider::OpenAiCompatible
        | TargetProvider::AzureOpenAi
        | TargetProvider::GitHubCopilot
        | TargetProvider::OpenWebUi => TargetCapabilities {
            supports_streaming: true,
            supports_tools: true,
            supports_conversation: true,
            supports_attachments: true,
            supports_memory: false,
            supports_agent: false,
        },
        TargetProvider::AnthropicClaude | TargetProvider::AwsBedrock => TargetCapabilities {
            supports_streaming: true,
            supports_tools: true,
            supports_conversation: true,
            supports_attachments: true,
            supports_memory: false,
            supports_agent: false,
        },
        TargetProvider::GoogleGemini => TargetCapabilities {
            supports_streaming: true,
            supports_tools: true,
            supports_conversation: true,
            supports_attachments: true,
            supports_memory: false,
            supports_agent: false,
        },
        TargetProvider::Dify | TargetProvider::Langflow => TargetCapabilities {
            supports_streaming: true,
            supports_tools: false,
            supports_conversation: true,
            supports_attachments: false,
            supports_memory: true,
            supports_agent: true,
        },
        TargetProvider::Mcp => TargetCapabilities {
            supports_streaming: false,
            supports_tools: true,
            supports_conversation: false,
            supports_attachments: false,
            supports_memory: false,
            supports_agent: true,
        },
        TargetProvider::GenericHttp | TargetProvider::GenericWebSocket => TargetCapabilities {
            supports_streaming: false,
            supports_tools: false,
            supports_conversation: false,
            supports_attachments: false,
            supports_memory: false,
            supports_agent: false,
        },
    }
}
