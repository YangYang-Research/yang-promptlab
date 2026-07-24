use crate::rules::{FingerprintRule, RuleMatcher, SignalKind};
use crate::types::AiProvider;

pub fn rules() -> Vec<FingerprintRule> {
    vec![
        FingerprintRule::new("anthropic.host", AiProvider::Anthropic, SignalKind::UrlHost, 0.40, RuleMatcher::HostContains("api.anthropic.com"), "Anthropic API host"),
        FingerprintRule::new("anthropic.path.messages", AiProvider::Anthropic, SignalKind::UrlPath, 0.35, RuleMatcher::PathRegex(r"(?i)/v1/messages/?$"), "Anthropic /v1/messages"),
        FingerprintRule::new("anthropic.header.version", AiProvider::Anthropic, SignalKind::Header, 0.45, RuleMatcher::HeaderPresent("anthropic-version"), "anthropic-version header"),
        FingerprintRule::new("anthropic.header.request-id", AiProvider::Anthropic, SignalKind::Header, 0.30, RuleMatcher::HeaderPresent("request-id"), "Anthropic request-id"),
        FingerprintRule::new("anthropic.body.auth_error", AiProvider::Anthropic, SignalKind::ResponseBody, 0.35, RuleMatcher::BodyJsonField { pointer: "/error/type", equals: Some("authentication_error") }, "authentication_error"),
        FingerprintRule::new("anthropic.body.invalid", AiProvider::Anthropic, SignalKind::ResponseBody, 0.30, RuleMatcher::BodyJsonField { pointer: "/error/type", equals: Some("invalid_request_error") }, "invalid_request_error"),
        FingerprintRule::new("anthropic.body.content", AiProvider::Anthropic, SignalKind::ResponseBody, 0.40, RuleMatcher::BodyJsonArrayContains { pointer: "/content", field: "type", value: "text" }, "content block type=text"),
    ]
}
