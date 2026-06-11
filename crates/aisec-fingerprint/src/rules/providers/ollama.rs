use crate::rules::{FingerprintRule, RuleMatcher, SignalKind};
use crate::types::AiProvider;

pub fn rules() -> Vec<FingerprintRule> {
    vec![
        FingerprintRule::new("ollama.path.tags", AiProvider::Ollama, SignalKind::UrlPath, 0.40, RuleMatcher::PathRegex(r"(?i)/api/tags/?$"), "/api/tags"),
        FingerprintRule::new("ollama.path.chat", AiProvider::Ollama, SignalKind::UrlPath, 0.35, RuleMatcher::PathRegex(r"(?i)/api/chat/?$"), "/api/chat"),
        FingerprintRule::new("ollama.path.generate", AiProvider::Ollama, SignalKind::UrlPath, 0.35, RuleMatcher::PathRegex(r"(?i)/api/generate/?$"), "/api/generate"),
        FingerprintRule::new("ollama.path.ps", AiProvider::Ollama, SignalKind::UrlPath, 0.30, RuleMatcher::PathRegex(r"(?i)/api/ps/?$"), "/api/ps"),
        FingerprintRule::new("ollama.port", AiProvider::Ollama, SignalKind::UrlHost, 0.20, RuleMatcher::UrlContains(":11434"), "default port 11434"),
        FingerprintRule::new("ollama.body.models", AiProvider::Ollama, SignalKind::ResponseBody, 0.45, RuleMatcher::BodyJsonArrayContains { pointer: "/models", field: "name", value: "llama" }, "models[].name"),
        FingerprintRule::new("ollama.body.model", AiProvider::Ollama, SignalKind::ResponseBody, 0.35, RuleMatcher::BodyJsonField { pointer: "/model", equals: None }, "model field"),
        FingerprintRule::new("ollama.body.done", AiProvider::Ollama, SignalKind::ResponseBody, 0.40, RuleMatcher::BodyJsonField { pointer: "/done_reason", equals: None }, "done_reason field"),
    ]
}
