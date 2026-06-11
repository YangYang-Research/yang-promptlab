use crate::rules::{FingerprintRule, RuleMatcher, SignalKind};
use crate::types::AiProvider;

pub fn rules() -> Vec<FingerprintRule> {
    vec![
        FingerprintRule::new("bedrock.host.runtime", AiProvider::Bedrock, SignalKind::UrlHost, 0.45, RuleMatcher::HostRegex(r"(?i)bedrock-runtime\.[a-z0-9-]+\.amazonaws\.com"), "Bedrock runtime host"),
        FingerprintRule::new("bedrock.host.control", AiProvider::Bedrock, SignalKind::UrlHost, 0.35, RuleMatcher::HostRegex(r"(?i)bedrock\.[a-z0-9-]+\.amazonaws\.com"), "Bedrock control host"),
        FingerprintRule::new("bedrock.path.invoke", AiProvider::Bedrock, SignalKind::UrlPath, 0.40, RuleMatcher::PathRegex(r"(?i)/model/[^/]+/(invoke|invoke-with-response-stream)"), "model invoke path"),
        FingerprintRule::new("bedrock.path.converse", AiProvider::Bedrock, SignalKind::UrlPath, 0.35, RuleMatcher::PathRegex(r"(?i)/converse(-stream)?/?$"), "Converse API path"),
        FingerprintRule::new("bedrock.header.requestid", AiProvider::Bedrock, SignalKind::Header, 0.30, RuleMatcher::HeaderPresent("x-amzn-requestid"), "x-amzn-requestid"),
        FingerprintRule::new("bedrock.header.bedrock", AiProvider::Bedrock, SignalKind::Header, 0.40, RuleMatcher::HeaderPresent("x-amz-bedrock"), "x-amz-bedrock header"),
        FingerprintRule::new("bedrock.body.coral", AiProvider::Bedrock, SignalKind::ResponseBody, 0.30, RuleMatcher::BodyContains(r#""__type":"com.amazon.coral.service"#), "AWS Coral error"),
        FingerprintRule::new("bedrock.body.output", AiProvider::Bedrock, SignalKind::ResponseBody, 0.35, RuleMatcher::BodyJsonField { pointer: "/output/message", equals: None }, "Converse output.message"),
    ]
}
