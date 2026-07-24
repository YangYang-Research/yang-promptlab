use crate::rules::{FingerprintRule, RuleMatcher, SignalKind};
use crate::types::AiProvider;

pub fn rules() -> Vec<FingerprintRule> {
    vec![
        FingerprintRule::new("gemini.host.gen", AiProvider::Gemini, SignalKind::UrlHost, 0.40, RuleMatcher::HostContains("generativelanguage.googleapis.com"), "Generative Language API"),
        FingerprintRule::new("gemini.host.vertex", AiProvider::Gemini, SignalKind::UrlHost, 0.35, RuleMatcher::HostRegex(r"(?i)aiplatform\.googleapis\.com"), "Vertex AI host"),
        FingerprintRule::new("gemini.path.generate", AiProvider::Gemini, SignalKind::UrlPath, 0.35, RuleMatcher::PathRegex(r"(?i)/v1(beta)?/models/[^/]+:(generateContent|streamGenerateContent)"), "generateContent path"),
        FingerprintRule::new("gemini.path.compat", AiProvider::Gemini, SignalKind::UrlPath, 0.30, RuleMatcher::PathRegex(r"(?i)/v1beta/openai/chat/completions"), "Gemini OpenAI compat"),
        FingerprintRule::new("gemini.header.goog", AiProvider::Gemini, SignalKind::Header, 0.35, RuleMatcher::HeaderPresent("x-goog-api-client"), "x-goog-api-client"),
        FingerprintRule::new("gemini.body.candidates", AiProvider::Gemini, SignalKind::ResponseBody, 0.45, RuleMatcher::BodyJsonField { pointer: "/candidates", equals: None }, "candidates field"),
        FingerprintRule::new("gemini.body.feedback", AiProvider::Gemini, SignalKind::ResponseBody, 0.35, RuleMatcher::BodyJsonField { pointer: "/promptFeedback", equals: None }, "promptFeedback field"),
    ]
}
