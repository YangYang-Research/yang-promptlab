use promptlab_harness::models::{HarnessKind, HttpMethod, TargetDescriptor, TargetSurface};

use crate::types::{TargetProfile, TargetProvider};

/// Map Target Profile provider to harness implementation.
pub fn harness_kind_for_profile(profile: &TargetProfile) -> HarnessKind {
    match profile.provider {
        TargetProvider::OpenAiCompatible
        | TargetProvider::OpenRouter
        | TargetProvider::AzureOpenAi
        | TargetProvider::GitHubCopilot
        | TargetProvider::OpenWebUi
        | TargetProvider::Langflow => HarnessKind::OpenAi,
        TargetProvider::AnthropicClaude => HarnessKind::Anthropic,
        TargetProvider::GoogleGemini => HarnessKind::Gemini,
        TargetProvider::Dify => HarnessKind::Dify,
        TargetProvider::AwsBedrock => HarnessKind::Bedrock,
        TargetProvider::Mcp => HarnessKind::Mcp,
        TargetProvider::GenericHttp => HarnessKind::Http,
        TargetProvider::GenericWebSocket => HarnessKind::WebSocket,
    }
}

pub fn target_surface_for_provider(provider: TargetProvider) -> TargetSurface {
    match provider {
        TargetProvider::OpenAiCompatible
        | TargetProvider::OpenRouter
        | TargetProvider::AzureOpenAi
        | TargetProvider::GitHubCopilot
        | TargetProvider::OpenWebUi
        | TargetProvider::Langflow => TargetSurface::OpenAiCompatible,
        TargetProvider::AnthropicClaude => TargetSurface::AnthropicCompatible,
        TargetProvider::GoogleGemini => TargetSurface::Gemini,
        TargetProvider::Dify => TargetSurface::Dify,
        TargetProvider::AwsBedrock => TargetSurface::Bedrock,
        TargetProvider::Mcp => TargetSurface::McpServer,
        TargetProvider::GenericHttp => TargetSurface::RestApi,
        TargetProvider::GenericWebSocket => TargetSurface::WebSocket,
    }
}

/// Build the descriptor every app surface (verify/scan/probe) uses for this profile.
pub fn descriptor_for_profile(profile: &TargetProfile) -> TargetDescriptor {
    TargetDescriptor {
        url: profile.full_url(),
        surface: target_surface_for_provider(profile.provider),
        method: HttpMethod::parse(profile.method.as_str()).unwrap_or(HttpMethod::Post),
        headers: profile.headers.clone(),
        body_template: Some(profile.request_template.clone()),
        ..TargetDescriptor::default()
    }
}

pub fn harness_id_for_profile(profile: &TargetProfile) -> &'static str {
    harness_kind_for_profile(profile).as_str()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates::template_for_provider;

    #[test]
    fn maps_protocol_specific_providers() {
        assert_eq!(
            harness_kind_for_profile(&template_for_provider(TargetProvider::AnthropicClaude)),
            HarnessKind::Anthropic
        );
        assert_eq!(
            harness_kind_for_profile(&template_for_provider(TargetProvider::GoogleGemini)),
            HarnessKind::Gemini
        );
        assert_eq!(
            harness_kind_for_profile(&template_for_provider(TargetProvider::Dify)),
            HarnessKind::Dify
        );
        assert_eq!(
            harness_kind_for_profile(&template_for_provider(TargetProvider::Mcp)),
            HarnessKind::Mcp
        );
        assert_eq!(
            harness_kind_for_profile(&template_for_provider(TargetProvider::GenericWebSocket)),
            HarnessKind::WebSocket
        );
        assert_eq!(
            descriptor_for_profile(&template_for_provider(TargetProvider::AnthropicClaude))
                .preferred_harness(),
            HarnessKind::Anthropic
        );
        assert_eq!(
            target_surface_for_provider(TargetProvider::Mcp),
            TargetSurface::McpServer
        );
    }
}
