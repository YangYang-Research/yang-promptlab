use aisec_harness::models::HarnessKind;

use crate::types::{TargetProfile, TargetProvider};

/// Map Target Profile provider to harness implementation.
pub fn harness_kind_for_profile(profile: &TargetProfile) -> HarnessKind {
    match profile.provider {
        TargetProvider::OpenAiCompatible
        | TargetProvider::AnthropicClaude
        | TargetProvider::GoogleGemini
        | TargetProvider::AzureOpenAi
        | TargetProvider::AwsBedrock
        | TargetProvider::GitHubCopilot
        | TargetProvider::OpenWebUi
        | TargetProvider::Dify
        | TargetProvider::Langflow => HarnessKind::OpenAi,
        TargetProvider::Mcp | TargetProvider::GenericHttp => HarnessKind::Http,
        TargetProvider::GenericWebSocket => HarnessKind::Http,
    }
}

pub fn harness_id_for_profile(profile: &TargetProfile) -> &'static str {
    harness_kind_for_profile(profile).as_str()
}
