use time::OffsetDateTime;
use tracing::{debug, instrument};

use crate::evaluator::evaluate_rule;
use crate::rules::{rule_catalog, FingerprintRule};
use crate::scoring::{apply_conflict_penalties, build_provider_fingerprint, ScoreAccumulator};
use crate::types::{
    AiProvider, DEFAULT_CONFIDENCE_THRESHOLD, FingerprintInput, FingerprintReport,
    ProviderFingerprint,
};

/// AI endpoint fingerprinting engine.
pub struct FingerprintEngine {
    rules: Vec<FingerprintRule>,
    threshold: f32,
}

impl FingerprintEngine {
    pub fn new() -> Self {
        Self {
            rules: rule_catalog(),
            threshold: DEFAULT_CONFIDENCE_THRESHOLD,
        }
    }

    pub fn with_threshold(threshold: f32) -> Self {
        Self {
            rules: rule_catalog(),
            threshold,
        }
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    pub fn rules_for(&self, provider: AiProvider) -> Vec<&FingerprintRule> {
        self.rules
            .iter()
            .filter(|r| r.provider == provider)
            .collect()
    }

    /// Fingerprint an endpoint from HTTP observation data.
    #[instrument(skip(self, input), fields(url = %input.url))]
    pub fn fingerprint(&self, input: &FingerprintInput) -> FingerprintReport {
        let mut per_provider: std::collections::HashMap<AiProvider, ScoreAccumulator> =
            std::collections::HashMap::new();

        for rule in &self.rules {
            if evaluate_rule(rule, input) {
                let entry = per_provider.entry(rule.provider).or_default();
                entry.raw_weight += rule.weight;
                entry.signals.push(crate::types::MatchedSignal {
                    provider: rule.provider,
                    rule_id: rule.id.to_string(),
                    description: rule.description.to_string(),
                    weight: rule.weight,
                });
            }
        }

        apply_conflict_penalties(&mut per_provider);

        let mut matches: Vec<ProviderFingerprint> = per_provider
            .into_iter()
            .map(|(provider, acc)| build_provider_fingerprint(provider, acc))
            .filter(|fp| fp.confidence >= self.threshold)
            .collect();

        matches.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let primary = matches.first().cloned();

        debug!(
            url = %input.url,
            match_count = matches.len(),
            primary = ?primary.as_ref().map(|p| p.provider.as_str()),
            "fingerprint complete"
        );

        FingerprintReport {
            url: input.url.clone(),
            matches,
            primary,
            analyzed_at: OffsetDateTime::now_utc(),
        }
    }

    /// Fingerprint multiple observations and merge by highest confidence per provider.
    pub fn fingerprint_batch(&self, inputs: &[FingerprintInput]) -> FingerprintReport {
        if inputs.is_empty() {
            return FingerprintReport {
                url: String::new(),
                matches: vec![],
                primary: None,
                analyzed_at: OffsetDateTime::now_utc(),
            };
        }

        let mut best: std::collections::HashMap<AiProvider, ProviderFingerprint> =
            std::collections::HashMap::new();

        for input in inputs {
            let report = self.fingerprint(input);
            for fp in report.matches {
                best.entry(fp.provider)
                    .and_modify(|existing| {
                        if fp.confidence > existing.confidence {
                            *existing = fp.clone();
                        }
                    })
                    .or_insert(fp);
            }
        }

        let mut matches: Vec<ProviderFingerprint> = best.into_values().collect();
        matches.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

        let url = inputs[0].url.clone();
        let primary = matches.first().cloned();

        FingerprintReport {
            url,
            matches,
            primary,
            analyzed_at: OffsetDateTime::now_utc(),
        }
    }
}

impl Default for FingerprintEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn input(url: &str, status: u16, headers: HashMap<String, String>, body: Option<&str>) -> FingerprintInput {
        FingerprintInput {
            url: url.into(),
            method: Some("POST".into()),
            status: Some(status),
            headers,
            body: body.map(str::to_string),
        }
    }

    #[test]
    fn detects_openai_api() {
        let engine = FingerprintEngine::new();
        let report = engine.fingerprint(&input(
            "https://api.openai.com/v1/chat/completions",
            401,
            HashMap::new(),
            Some(r#"{"error":{"type":"invalid_request_error","message":"missing key"}}"#),
        ));
        let primary = report.primary.expect("primary");
        assert_eq!(primary.provider, AiProvider::OpenAi);
        assert!(primary.confidence >= 0.7);
    }

    #[test]
    fn detects_anthropic_api() {
        let engine = FingerprintEngine::new();
        let mut headers = HashMap::new();
        headers.insert("anthropic-version".into(), "2023-06-01".into());
        headers.insert("request-id".into(), "req_123".into());
        let report = engine.fingerprint(&input(
            "https://api.anthropic.com/v1/messages",
            401,
            headers,
            Some(r#"{"error":{"type":"authentication_error","message":"invalid key"}}"#),
        ));
        assert_eq!(report.primary.unwrap().provider, AiProvider::Anthropic);
    }

    #[test]
    fn detects_gemini_api() {
        let engine = FingerprintEngine::new();
        let report = engine.fingerprint(&input(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-pro:generateContent",
            200,
            HashMap::new(),
            Some(r#"{"candidates":[{"content":{"parts":[{"text":"hi"}]}}],"promptFeedback":{}}"#),
        ));
        assert_eq!(report.primary.unwrap().provider, AiProvider::Gemini);
    }

    #[test]
    fn detects_bedrock_api() {
        let engine = FingerprintEngine::new();
        let mut headers = HashMap::new();
        headers.insert("x-amzn-requestid".into(), "abc".into());
        let report = engine.fingerprint(&input(
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/anthropic.claude-3-sonnet/invoke",
            403,
            headers,
            None,
        ));
        assert_eq!(report.primary.unwrap().provider, AiProvider::Bedrock);
    }

    #[test]
    fn detects_azure_openai() {
        let engine = FingerprintEngine::new();
        let mut headers = HashMap::new();
        headers.insert("x-ms-client-request-id".into(), "ms-1".into());
        let report = engine.fingerprint(&input(
            "https://myresource.openai.azure.com/openai/deployments/gpt-4o/chat/completions?api-version=2024-02-01",
            404,
            headers,
            Some(r#"{"error":{"code":"DeploymentNotFound","message":"not found"}}"#),
        ));
        assert_eq!(report.primary.unwrap().provider, AiProvider::AzureOpenAi);
    }

    #[test]
    fn detects_ollama_api() {
        let engine = FingerprintEngine::new();
        let report = engine.fingerprint(&input(
            "http://127.0.0.1:11434/api/tags",
            200,
            HashMap::new(),
            Some(r#"{"models":[{"name":"llama3:latest","model":"llama3:latest"}]}"#),
        ));
        assert_eq!(report.primary.unwrap().provider, AiProvider::Ollama);
    }

    #[test]
    fn detects_litellm_proxy() {
        let engine = FingerprintEngine::new();
        let mut headers = HashMap::new();
        headers.insert("x-litellm-model-id".into(), "gpt-4".into());
        headers.insert("x-litellm-provider".into(), "openai".into());
        let report = engine.fingerprint(&input(
            "https://proxy.internal/v1/chat/completions",
            502,
            headers,
            Some(r#"{"error":{"type":"litellm_error","message":"LiteLLM rate limit"}}"#),
        ));
        assert_eq!(report.primary.unwrap().provider, AiProvider::LiteLlm);
    }

    #[test]
    fn detects_vllm_server() {
        let engine = FingerprintEngine::new();
        let mut headers = HashMap::new();
        headers.insert("server".into(), "uvicorn/vllm-0.6.0".into());
        let report = engine.fingerprint(&input(
            "http://gpu-cluster:8000/health",
            200,
            headers,
            Some(r#"{"status":"ok","vllm":"0.6.0"}"#),
        ));
        assert_eq!(report.primary.unwrap().provider, AiProvider::Vllm);
    }

    #[test]
    fn below_threshold_excluded() {
        let engine = FingerprintEngine::with_threshold(0.90);
        let report = engine.fingerprint(&input(
            "https://example.com/v1/chat/completions",
            200,
            HashMap::new(),
            None,
        ));
        assert!(report.primary.is_none());
    }

    #[test]
    fn rule_catalog_covers_all_providers() {
        let engine = FingerprintEngine::new();
        for provider in AiProvider::all() {
            assert!(
                !engine.rules_for(*provider).is_empty(),
                "missing rules for {}",
                provider.display_name()
            );
        }
    }
}
