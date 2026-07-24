use crate::rules::{FingerprintRule, RuleMatcher, SignalKind};
use crate::types::AiProvider;

pub fn rules() -> Vec<FingerprintRule> {
    vec![
        FingerprintRule::new("azure.host", AiProvider::AzureOpenAi, SignalKind::UrlHost, 0.45, RuleMatcher::HostRegex(r"(?i)\.openai\.azure\.com"), "Azure OpenAI host"),
        FingerprintRule::new("azure.path.deployment", AiProvider::AzureOpenAi, SignalKind::UrlPath, 0.40, RuleMatcher::PathRegex(r"(?i)/openai/deployments/[^/]+/(chat/completions|completions|embeddings)"), "deployment inference path"),
        FingerprintRule::new("azure.path.list", AiProvider::AzureOpenAi, SignalKind::UrlPath, 0.30, RuleMatcher::PathRegex(r"(?i)/openai/deployments/?"), "deployments list"),
        FingerprintRule::new("azure.header.ms", AiProvider::AzureOpenAi, SignalKind::Header, 0.35, RuleMatcher::HeaderPresent("x-ms-client-request-id"), "x-ms-client-request-id"),
        FingerprintRule::new("azure.header.azureml", AiProvider::AzureOpenAi, SignalKind::Header, 0.30, RuleMatcher::HeaderPresent("azureml-model-deployment"), "azureml-model-deployment"),
        FingerprintRule::new("azure.body.notfound", AiProvider::AzureOpenAi, SignalKind::ResponseBody, 0.30, RuleMatcher::BodyJsonField { pointer: "/error/code", equals: Some("DeploymentNotFound") }, "DeploymentNotFound"),
        FingerprintRule::new("azure.body.inner", AiProvider::AzureOpenAi, SignalKind::ResponseBody, 0.25, RuleMatcher::BodyJsonField { pointer: "/error/innererror", equals: None }, "innererror object"),
    ]
}
