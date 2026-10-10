use promptlab_harness::{
    AuthMaterial, HarnessKind, HttpMethod, TargetDescriptor, TargetSurface,
};

use super::RemoteAdapterSettings;
use crate::config::InferenceProvider;

/// Map vault / third-party settings onto a harness descriptor (no HTTP here).
pub fn descriptor_from_remote(settings: &RemoteAdapterSettings) -> TargetDescriptor {
    let mut descriptor = TargetDescriptor::default();
    descriptor.method = HttpMethod::Post;
    descriptor.auth = auth_from_remote(settings);
    let base = settings
        .base_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(|url| url.trim_end_matches('/').to_string())
        .unwrap_or_else(|| default_base(settings));
    match settings.provider {
        InferenceProvider::Anthropic => {
            descriptor.surface = TargetSurface::AnthropicCompatible;
            descriptor.url = join_url(&base, "/messages");
        }
        InferenceProvider::Gemini => {
            descriptor.surface = TargetSurface::Gemini;
            descriptor.url = format!(
                "{}/models/{}:generateContent",
                base.trim_end_matches('/'),
                settings.model
            );
        }
        InferenceProvider::Bedrock => {
            descriptor.surface = TargetSurface::Bedrock;
            let region = settings
                .aws_region
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("us-east-1");
            descriptor.url = format!(
                "https://bedrock-runtime.{region}.amazonaws.com/model/{}/converse",
                settings.model.trim()
            );
        }
        InferenceProvider::Deterministic => {
            descriptor.surface = TargetSurface::OpenAiCompatible;
            descriptor.url = if base.is_empty() {
                String::new()
            } else {
                join_url(&base, "/chat/completions")
            };
        }
        _ => {
            descriptor.surface = TargetSurface::OpenAiCompatible;
            descriptor.url = join_url(&base, "/chat/completions");
        }
    }
    let _ = descriptor.preferred_harness();
    let _ = HarnessKind::OpenAi;
    descriptor
}

pub fn auth_from_remote(settings: &RemoteAdapterSettings) -> AuthMaterial {
    let mut auth = AuthMaterial::default();
    match settings.provider {
        InferenceProvider::Anthropic => {
            auth.api_key = Some(settings.api_key.clone());
            auth.api_key_header = Some("x-api-key".into());
        }
        InferenceProvider::YangYang => {
            // YIE: x-yang-api-token: Basic <token>. User stores bare token; we add Basic.
            auth.api_key = Some(yangyang_api_token_header_value(&settings.api_key));
            auth.api_key_header = Some("x-yang-api-token".into());
        }
        InferenceProvider::Gemini => {
            auth.api_key = Some(settings.api_key.clone());
            auth.query_key_name = Some("key".into());
            auth.query_key_value = Some(settings.api_key.clone());
        }
        InferenceProvider::Bedrock => {
            auth.aws_access_key_id = Some(settings.api_key.clone());
            auth.aws_secret_access_key = settings.aws_secret_access_key.clone();
            auth.aws_session_token = settings.aws_session_token.clone();
            auth.aws_region = settings.aws_region.clone();
            auth.aws_service = Some("bedrock".into());
        }
        _ => {
            auth.bearer_token = Some(settings.api_key.clone());
        }
    }
    auth
}

fn default_base(settings: &RemoteAdapterSettings) -> String {
    match settings.provider {
        InferenceProvider::OpenAi => "https://api.openai.com/v1".into(),
        InferenceProvider::Anthropic => "https://api.anthropic.com/v1".into(),
        InferenceProvider::Gemini => "https://generativelanguage.googleapis.com/v1beta".into(),
        InferenceProvider::OpenRouter => "https://openrouter.ai/api/v1".into(),
        InferenceProvider::Nvidia => "https://integrate.api.nvidia.com/v1".into(),
        InferenceProvider::YangYang => "https://inference.yyng.icu/v1".into(),
        InferenceProvider::Azure => String::new(),
        InferenceProvider::Bedrock => {
            let region = settings
                .aws_region
                .as_deref()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or("us-east-1");
            format!("https://bedrock-runtime.{region}.amazonaws.com")
        }
        InferenceProvider::Deterministic => String::new(),
    }
}

fn join_url(base: &str, path: &str) -> String {
    if base.is_empty() {
        return path.to_string();
    }
    if base.ends_with(path) || base.contains("/chat/completions") || base.contains("/messages") {
        return base.to_string();
    }
    format!("{}{}", base.trim_end_matches('/'), path)
}

/// Normalize vault value to `Basic <token>` for the x-yang-api-token header.
fn yangyang_api_token_header_value(api_key: &str) -> String {
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.len() >= 6 && trimmed[..6].eq_ignore_ascii_case("basic ") {
        return format!("Basic {}", trimmed[6..].trim());
    }
    format!("Basic {trimmed}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::RemoteAdapterSettings;

    #[test]
    fn openai_descriptor_uses_chat_completions() {
        let settings = RemoteAdapterSettings {
            provider: InferenceProvider::OpenAi,
            model: "gpt-4o".into(),
            base_url: None,
            api_key: "sk-test".into(),
            aws_secret_access_key: None,
            aws_region: None,
            aws_session_token: None,
        };
        let descriptor = descriptor_from_remote(&settings);
        assert_eq!(descriptor.surface, TargetSurface::OpenAiCompatible);
        assert!(descriptor.url.ends_with("/chat/completions"));
        assert_eq!(descriptor.auth.bearer_token.as_deref(), Some("sk-test"));
    }

    #[test]
    fn yangyang_uses_x_yang_api_token_and_default_base() {
        let settings = RemoteAdapterSettings {
            provider: InferenceProvider::YangYang,
            model: "@cf/meta/llama-3.1-8b-instruct".into(),
            base_url: None,
            api_key: "test-token".into(),
            aws_secret_access_key: None,
            aws_region: None,
            aws_session_token: None,
        };
        let descriptor = descriptor_from_remote(&settings);
        assert_eq!(descriptor.surface, TargetSurface::OpenAiCompatible);
        assert_eq!(
            descriptor.url,
            "https://inference.yyng.icu/v1/chat/completions"
        );
        assert_eq!(descriptor.auth.api_key.as_deref(), Some("Basic test-token"));
        assert_eq!(
            descriptor.auth.api_key_header.as_deref(),
            Some("x-yang-api-token")
        );
        assert!(descriptor.auth.bearer_token.is_none());
    }

    #[test]
    fn yangyang_keeps_basic_prefix_if_user_already_typed_it() {
        assert_eq!(
            yangyang_api_token_header_value("Basic abc123"),
            "Basic abc123"
        );
        assert_eq!(
            yangyang_api_token_header_value("basic  abc123"),
            "Basic abc123"
        );
        assert_eq!(yangyang_api_token_header_value("abc123"), "Basic abc123");
    }
}
