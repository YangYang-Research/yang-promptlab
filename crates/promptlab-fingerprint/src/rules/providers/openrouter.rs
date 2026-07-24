use crate::rules::{FingerprintRule, RuleMatcher, SignalKind};
use crate::types::AiProvider;

pub fn rules() -> Vec<FingerprintRule> {
    vec![
        FingerprintRule::new(
            "openrouter.host",
            AiProvider::OpenRouter,
            SignalKind::UrlHost,
            0.40,
            RuleMatcher::HostContains("openrouter.ai"),
            "Host is openrouter.ai",
        ),
        FingerprintRule::new(
            "openrouter.path.chat",
            AiProvider::OpenRouter,
            SignalKind::UrlPath,
            0.30,
            RuleMatcher::PathRegex(r"(?i)/api/v1/chat/completions/?$"),
            "OpenRouter chat completions path",
        ),
        FingerprintRule::new(
            "openrouter.header.referer",
            AiProvider::OpenRouter,
            SignalKind::Header,
            0.35,
            RuleMatcher::HeaderPresent("http-referer"),
            "OpenRouter HTTP-Referer header",
        ),
        FingerprintRule::new(
            "openrouter.body.error",
            AiProvider::OpenRouter,
            SignalKind::ResponseBody,
            0.30,
            RuleMatcher::BodyContains("openrouter"),
            "OpenRouter referenced in response body",
        ),
        FingerprintRule::new(
            "openrouter.body.models",
            AiProvider::OpenRouter,
            SignalKind::ResponseBody,
            0.35,
            RuleMatcher::BodyJsonArrayContains {
                pointer: "/data",
                field: "id",
                value: "openrouter/",
            },
            "OpenRouter model id prefix in models list",
        ),
    ]
}
