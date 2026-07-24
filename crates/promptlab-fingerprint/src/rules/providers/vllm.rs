use crate::rules::{FingerprintRule, RuleMatcher, SignalKind};
use crate::types::AiProvider;

pub fn rules() -> Vec<FingerprintRule> {
    vec![
        FingerprintRule::new("vllm.header.server", AiProvider::Vllm, SignalKind::Header, 0.50, RuleMatcher::HeaderContains { name: "server", value: "vllm" }, "Server: vllm"),
        FingerprintRule::new("vllm.path.health", AiProvider::Vllm, SignalKind::UrlPath, 0.40, RuleMatcher::PathRegex(r"(?i)/health/?$"), "/health"),
        FingerprintRule::new("vllm.path.metrics", AiProvider::Vllm, SignalKind::UrlPath, 0.35, RuleMatcher::PathRegex(r"(?i)/metrics/?$"), "/metrics"),
        FingerprintRule::new("vllm.path.chat", AiProvider::Vllm, SignalKind::UrlPath, 0.25, RuleMatcher::PathRegex(r"(?i)/v1/chat/completions/?$"), "OpenAI-compat chat path"),
        FingerprintRule::new("vllm.body.health", AiProvider::Vllm, SignalKind::ResponseBody, 0.45, RuleMatcher::BodyContains(r#""status":"ok"#), "health status ok"),
        FingerprintRule::new("vllm.body.vllm", AiProvider::Vllm, SignalKind::ResponseBody, 0.40, RuleMatcher::BodyContains(r#""vllm""#), "vllm in JSON body"),
        FingerprintRule::new("vllm.body.usage", AiProvider::Vllm, SignalKind::ResponseBody, 0.20, RuleMatcher::BodyJsonField { pointer: "/usage/prompt_tokens", equals: None }, "usage.prompt_tokens"),
    ]
}
