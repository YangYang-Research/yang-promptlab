use crate::rules::{FingerprintRule, RuleMatcher, SignalKind};
use crate::types::AiProvider;

pub fn rules() -> Vec<FingerprintRule> {
    vec![
        FingerprintRule::new("litellm.header.model", AiProvider::LiteLlm, SignalKind::Header, 0.50, RuleMatcher::HeaderPresent("x-litellm-model-id"), "x-litellm-model-id"),
        FingerprintRule::new("litellm.header.provider", AiProvider::LiteLlm, SignalKind::Header, 0.45, RuleMatcher::HeaderPresent("x-litellm-provider"), "x-litellm-provider"),
        FingerprintRule::new("litellm.header.call", AiProvider::LiteLlm, SignalKind::Header, 0.40, RuleMatcher::HeaderPresent("x-litellm-call-id"), "x-litellm-call-id"),
        FingerprintRule::new("litellm.body.name", AiProvider::LiteLlm, SignalKind::ResponseBody, 0.35, RuleMatcher::BodyContains("LiteLLM"), "LiteLLM in body"),
        FingerprintRule::new("litellm.body.error", AiProvider::LiteLlm, SignalKind::ResponseBody, 0.30, RuleMatcher::BodyJsonField { pointer: "/error/type", equals: Some("litellm_error") }, "litellm_error type"),
        FingerprintRule::new("litellm.path.health", AiProvider::LiteLlm, SignalKind::UrlPath, 0.25, RuleMatcher::PathRegex(r"(?i)/health/litellm/?$"), "/health/litellm"),
    ]
}
