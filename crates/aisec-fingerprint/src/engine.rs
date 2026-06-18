use std::collections::{HashMap, HashSet};

use time::OffsetDateTime;
use tracing::{debug, instrument};

use crate::evaluator::evaluate_rule;
use crate::recommendations::{
    aggregate_confidence, generate_attack_recommendations, technologies_from_providers,
};
use crate::rules::{rule_catalog, stack_rule_catalog, FingerprintRule, StackRule, StackTarget};
use crate::scoring::{apply_conflict_penalties, build_provider_fingerprint, ScoreAccumulator};
use crate::types::{
    AiProvider, COMPONENT_CONFIDENCE_THRESHOLD, DEFAULT_CONFIDENCE_THRESHOLD,
    DetectedComponent, DetectedFramework, FingerprintInput, FingerprintMethod,
    FingerprintReport, FRAMEWORK_CONFIDENCE_THRESHOLD, ProviderFingerprint, StackFingerprintReport,
    StackSignal,
};

/// AI endpoint fingerprinting engine.
pub struct FingerprintEngine {
    rules: Vec<FingerprintRule>,
    stack_rules: Vec<StackRule>,
    threshold: f32,
}

impl FingerprintEngine {
    pub fn new() -> Self {
        Self {
            rules: rule_catalog(),
            stack_rules: stack_rule_catalog(),
            threshold: DEFAULT_CONFIDENCE_THRESHOLD,
        }
    }

    pub fn with_threshold(threshold: f32) -> Self {
        Self {
            rules: rule_catalog(),
            stack_rules: stack_rule_catalog(),
            threshold,
        }
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len() + self.stack_rules.len()
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
        let mut per_provider: HashMap<AiProvider, ScoreAccumulator> = HashMap::new();

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
            .map(|(provider, acc)| build_provider_fingerprint(provider, acc, input))
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

    /// Full AI stack fingerprint: providers, frameworks, components, and attack recommendations.
    pub fn fingerprint_stack(&self, input: &FingerprintInput) -> StackFingerprintReport {
        let provider_report = if input
            .kind_hint
            .as_deref()
            .is_some_and(|k| k == "openapi")
            && input
                .body
                .as_deref()
                .and_then(|b| serde_json::from_str::<serde_json::Value>(b).ok())
                .is_some()
        {
            self.fingerprint_openapi(
                &serde_json::from_str(input.body.as_deref().unwrap_or("{}")).unwrap_or_default(),
            )
        } else {
            self.fingerprint(input)
        };

        let mut framework_scores: HashMap<crate::types::AgentFramework, StackScoreAccumulator> =
            HashMap::new();
        let mut component_scores: HashMap<crate::types::AiComponent, StackScoreAccumulator> =
            HashMap::new();
        let mut methods_used = HashSet::new();

        infer_methods_from_input(input, &mut methods_used);

        for rule in &self.stack_rules {
            if !evaluate_rule(&rule_to_fingerprint(rule), input) {
                continue;
            }
            methods_used.insert(rule.method.as_str().to_string());
            match rule.target {
                StackTarget::Framework(framework) => {
                    let entry = framework_scores.entry(framework).or_default();
                    entry.raw_weight += rule.weight;
                    entry.stack_signals.push(StackSignal {
                        rule_id: rule.id.to_string(),
                        description: rule.description.to_string(),
                        weight: rule.weight,
                        method: rule.method,
                    });
                }
                StackTarget::Component(component) => {
                    let entry = component_scores.entry(component).or_default();
                    entry.raw_weight += rule.weight;
                    entry.stack_signals.push(StackSignal {
                        rule_id: rule.id.to_string(),
                        description: rule.description.to_string(),
                        weight: rule.weight,
                        method: rule.method,
                    });
                }
            }
        }

        let technologies = technologies_from_providers(&provider_report.matches);
        let agent_frameworks = framework_scores
            .into_iter()
            .filter_map(|(framework, acc)| {
                let confidence = compute_stack_confidence(&acc);
                if confidence < FRAMEWORK_CONFIDENCE_THRESHOLD {
                    return None;
                }
                Some(DetectedFramework {
                    framework,
                    name: framework.display_name().into(),
                    confidence,
                    signals: acc
                        .stack_signals
                        .iter()
                        .map(|s| s.description.clone())
                        .collect(),
                })
            })
            .collect::<Vec<_>>();

        let ai_components = component_scores
            .into_iter()
            .filter_map(|(component, acc)| {
                let confidence = compute_stack_confidence(&acc);
                if confidence < COMPONENT_CONFIDENCE_THRESHOLD {
                    return None;
                }
                Some(DetectedComponent {
                    component,
                    name: component.display_name().into(),
                    confidence,
                    signals: acc
                        .stack_signals
                        .iter()
                        .map(|s| s.description.clone())
                        .collect(),
                })
            })
            .collect::<Vec<_>>();

        let mut agent_frameworks = agent_frameworks;
        agent_frameworks.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut ai_components = ai_components;
        ai_components.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let confidence = aggregate_confidence(&technologies, &agent_frameworks, &ai_components);

        let mut methods_used: Vec<String> = methods_used.into_iter().collect();
        methods_used.sort();

        let mut report = StackFingerprintReport {
            url: input.url.clone(),
            confidence,
            technologies,
            agent_frameworks,
            ai_components,
            provider_report,
            attack_recommendations: vec![],
            methods_used,
            platform_profile: Default::default(),
            analyzed_at: OffsetDateTime::now_utc(),
        };
        report.attack_recommendations = generate_attack_recommendations(&report);
        report.platform_profile = crate::profile::build_platform_profile(&report, input);
        report
    }

    /// Detect AI providers from an OpenAPI/Swagger specification.
    #[instrument(skip(self, spec))]
    pub fn fingerprint_openapi(&self, spec: &serde_json::Value) -> FingerprintReport {
        let inputs = crate::openapi::inputs_from_openapi(spec);
        self.fingerprint_batch(&inputs)
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

        let mut best: HashMap<AiProvider, ProviderFingerprint> = HashMap::new();

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

#[derive(Debug, Default)]
struct StackScoreAccumulator {
    raw_weight: f32,
    stack_signals: Vec<StackSignal>,
}

fn compute_stack_confidence(acc: &StackScoreAccumulator) -> f32 {
    if acc.stack_signals.is_empty() {
        return 0.0;
    }
    let mut score = 1.0 - (-acc.raw_weight).exp();
    if acc.stack_signals.len() == 1 && acc.raw_weight >= 0.35 {
        score = score.max(0.65);
    }
    score.min(1.0)
}

fn rule_to_fingerprint(rule: &StackRule) -> FingerprintRule {
    FingerprintRule {
        id: rule.id,
        provider: AiProvider::OpenAi,
        kind: rule.kind,
        weight: rule.weight,
        matcher: rule.matcher.clone(),
        description: rule.description,
    }
}

fn infer_methods_from_input(input: &FingerprintInput, methods: &mut HashSet<String>) {
    if !input.headers.is_empty() {
        methods.insert(FingerprintMethod::Headers.as_str().into());
    }
    if input.body.is_some() {
        methods.insert(FingerprintMethod::Responses.as_str().into());
    }
    methods.insert(FingerprintMethod::KnownRoutes.as_str().into());

    if input.kind_hint.as_deref() == Some("openapi") {
        methods.insert(FingerprintMethod::OpenApi.as_str().into());
    }
    if input.kind_hint.as_deref() == Some("graphql") {
        methods.insert(FingerprintMethod::GraphQl.as_str().into());
    }
    if input.kind_hint.as_deref() == Some("javascript") {
        methods.insert(FingerprintMethod::JavaScript.as_str().into());
    }
    if input.content_type.as_deref().is_some_and(|ct| ct.contains("javascript")) {
        methods.insert(FingerprintMethod::JavaScript.as_str().into());
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
            content_type: None,
            kind_hint: None,
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
    fn detects_openrouter_api() {
        let engine = FingerprintEngine::new();
        let report = engine.fingerprint(&input(
            "https://openrouter.ai/api/v1/chat/completions",
            401,
            HashMap::new(),
            None,
        ));
        assert_eq!(report.primary.unwrap().provider, AiProvider::OpenRouter);
    }

    #[test]
    fn stack_detects_langchain_and_recommends_tool_abuse() {
        let engine = FingerprintEngine::new();
        let report = engine.fingerprint_stack(&input(
            "https://example.com/invoke",
            200,
            HashMap::new(),
            Some(r#"{"output":"ok","langchain":true}"#),
        ));
        assert!(report
            .agent_frameworks
            .iter()
            .any(|f| f.framework == crate::types::AgentFramework::LangChain));
        assert!(report
            .attack_recommendations
            .iter()
            .any(|r| r.category == "tool_abuse"));
    }

    #[test]
    fn stack_detects_mcp_server() {
        let engine = FingerprintEngine::new();
        let report = engine.fingerprint_stack(&input(
            "https://example.com/mcp",
            200,
            HashMap::new(),
            Some(r#"{"jsonrpc":"2.0","result":{"tools":[{"name":"read_file"}]}}"#),
        ));
        assert!(report
            .ai_components
            .iter()
            .any(|c| c.component == crate::types::AiComponent::McpServer));
        assert!(report
            .attack_recommendations
            .iter()
            .any(|r| r.category == "mcp_abuse"));
    }

    #[test]
    fn stack_detects_langflow() {
        let engine = FingerprintEngine::new();
        let report = engine.fingerprint_stack(&input(
            "https://example.com/api/v1/run",
            200,
            HashMap::new(),
            Some(r#"{"outputs":[],"langflow":true}"#),
        ));
        assert!(report
            .agent_frameworks
            .iter()
            .any(|f| f.framework == crate::types::AgentFramework::Langflow));
        assert_eq!(report.platform_profile.platform, "langflow");
    }

    #[test]
    fn stack_detects_librechat() {
        let engine = FingerprintEngine::new();
        let report = engine.fingerprint_stack(&input(
            "https://example.com/api/ask",
            200,
            HashMap::new(),
            Some(r#"{"text":"ok","LibreChat":true}"#),
        ));
        assert!(report
            .agent_frameworks
            .iter()
            .any(|f| f.framework == crate::types::AgentFramework::LibreChat));
        assert!(report.platform_profile.memory_enabled);
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
