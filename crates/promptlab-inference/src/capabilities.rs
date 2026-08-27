use serde::{Deserialize, Serialize};

/// Model/runtime capability flags exposed to AI features.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelCapabilities {
    pub supports_chat: bool,
    pub supports_streaming: bool,
    pub supports_embedding: bool,
    pub supports_vision: bool,
    pub supports_json: bool,
    pub supports_tool_calling: bool,
    pub supports_thinking: bool,
    pub supports_reasoning: bool,
    pub supports_function_calling: bool,
    pub supports_images: bool,
}

impl ModelCapabilities {
    pub fn deterministic() -> Self {
        Self {
            supports_json: true,
            ..Self::default()
        }
    }

    pub fn from_remote(provider: &str) -> Self {
        let mut caps = Self {
            supports_chat: true,
            supports_streaming: true,
            supports_json: true,
            supports_tool_calling: matches!(
                provider,
                "openai" | "anthropic" | "gemini" | "nvidia" | "openrouter" | "azure" | "custom"
            ),
            supports_function_calling: matches!(
                provider,
                "openai" | "anthropic" | "nvidia" | "openrouter" | "azure" | "custom"
            ),
            ..Self::default()
        };
        if provider == "gemini" {
            caps.supports_vision = true;
            caps.supports_images = true;
        }
        caps
    }
}
