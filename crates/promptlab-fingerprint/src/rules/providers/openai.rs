use crate::rules::{FingerprintRule, RuleMatcher, SignalKind};
use crate::types::AiProvider;

pub fn rules() -> Vec<FingerprintRule> {
    vec![
        FingerprintRule::new("openai.host", AiProvider::OpenAi, SignalKind::UrlHost, 0.35, RuleMatcher::HostContains("api.openai.com"), "Host is api.openai.com"),
        FingerprintRule::new("openai.path.chat", AiProvider::OpenAi, SignalKind::UrlPath, 0.30, RuleMatcher::PathRegex(r"(?i)/v1/chat/completions/?$"), "Path is /v1/chat/completions"),
        FingerprintRule::new("openai.path.models", AiProvider::OpenAi, SignalKind::UrlPath, 0.25, RuleMatcher::PathRegex(r"(?i)/v1/models/?$"), "Path is /v1/models"),
        FingerprintRule::new("openai.path.embeddings", AiProvider::OpenAi, SignalKind::UrlPath, 0.25, RuleMatcher::PathRegex(r"(?i)/v1/embeddings/?$"), "Path is /v1/embeddings"),
        FingerprintRule::new("openai.header.org", AiProvider::OpenAi, SignalKind::Header, 0.40, RuleMatcher::HeaderPresent("openai-organization"), "OpenAI-Organization header"),
        FingerprintRule::new("openai.header.version", AiProvider::OpenAi, SignalKind::Header, 0.35, RuleMatcher::HeaderPresent("openai-version"), "OpenAI-Version header"),
        FingerprintRule::new("openai.body.error", AiProvider::OpenAi, SignalKind::ResponseBody, 0.35, RuleMatcher::BodyJsonField { pointer: "/error/type", equals: Some("invalid_request_error") }, "OpenAI error shape"),
        FingerprintRule::new("openai.body.models", AiProvider::OpenAi, SignalKind::ResponseBody, 0.40, RuleMatcher::BodyJsonArrayContains { pointer: "/data", field: "object", value: "model" }, "OpenAI models list"),
        FingerprintRule::new("openai.status.auth", AiProvider::OpenAi, SignalKind::StatusCode, 0.15, RuleMatcher::StatusIn(&[401, 403]), "401/403 on OpenAI endpoint"),
    ]
}
