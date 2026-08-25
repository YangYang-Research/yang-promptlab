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
        InferenceProvider::Ollama => {
            descriptor.surface = TargetSurface::Ollama;
            descriptor.url = join_url(&base, "/v1/chat/completions");
        }
        InferenceProvider::LlamaCpp | InferenceProvider::Deterministic => {
            // LlamaCpp in-process path removed; treat as OpenAI-compatible if base_url set.
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
        InferenceProvider::Azure => String::new(),
        InferenceProvider::Bedrock => {
            let region = settings
                .aws_region
                .as_deref()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or("us-east-1");
            format!("https://bedrock-runtime.{region}.amazonaws.com")
        }
        InferenceProvider::Ollama => "http://127.0.0.1:11434".into(),
        InferenceProvider::LlamaCpp => String::new(),
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
}
