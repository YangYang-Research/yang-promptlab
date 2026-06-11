use std::collections::HashMap;

use crate::types::{AiProvider, ApiStyle, MatchedSignal, ProviderFingerprint};

/// Raw score accumulator before normalization.
#[derive(Debug, Default)]
pub struct ScoreAccumulator {
    pub raw_weight: f32,
    pub signals: Vec<MatchedSignal>,
}

/// Computes normalized confidence from matched rule weights.
pub fn compute_confidence(accumulator: &ScoreAccumulator) -> f32 {
    if accumulator.signals.is_empty() {
        return 0.0;
    }

    let raw = accumulator.raw_weight;
    // Saturating curve: approaches 1.0 as evidence accumulates
    let mut score = 1.0 - (-raw).exp();

    // Reward diverse signal types (host + body + header)
    let kinds: std::collections::HashSet<_> = accumulator
        .signals
        .iter()
        .map(|s| signal_kind_key(&s.rule_id))
        .collect();
    score += (kinds.len().saturating_sub(1) as f32 * 0.04).min(0.12);

    // Single very strong signal (e.g. provider-specific header)
    if accumulator.signals.len() == 1 && raw >= 0.40 {
        score = score.max(0.72);
    }

    score.min(1.0)
}

fn signal_kind_key(rule_id: &str) -> &str {
    rule_id.split('.').nth(1).unwrap_or(rule_id)
}

/// Apply cross-provider conflict penalties when generic OpenAI paths match multiple vendors.
pub fn apply_conflict_penalties(scores: &mut HashMap<AiProvider, ScoreAccumulator>) {
    let openai_compat = [
        AiProvider::OpenAi,
        AiProvider::Vllm,
        AiProvider::LiteLlm,
        AiProvider::Ollama,
    ];

    let has_distinctive = |provider: AiProvider| {
        scores.get(&provider).is_some_and(|acc| {
            acc.signals.iter().any(|s| {
                s.rule_id.contains("header")
                    || s.rule_id.contains("host")
                    || s.weight >= 0.40
            })
        })
    };

    let generic_only: Vec<AiProvider> = openai_compat
        .into_iter()
        .filter(|p| scores.contains_key(p) && !has_distinctive(*p))
        .collect();

    if generic_only.len() >= 2 {
        for provider in generic_only {
            if let Some(acc) = scores.get_mut(&provider) {
                acc.raw_weight *= 0.75;
            }
        }
    }

    // Azure and OpenAI: penalize OpenAI if Azure host matched
    if scores.contains_key(&AiProvider::AzureOpenAi) && scores.contains_key(&AiProvider::OpenAi) {
        if let Some(azure) = scores.get(&AiProvider::AzureOpenAi) {
            if azure.signals.iter().any(|s| s.rule_id.starts_with("azure.host")) {
                if let Some(openai) = scores.get_mut(&AiProvider::OpenAi) {
                    openai.raw_weight *= 0.5;
                }
            }
        }
    }

    // vLLM vs OpenAI: boost vLLM if server header matched
    if scores.contains_key(&AiProvider::Vllm) {
        let vllm_distinct = scores
            .get(&AiProvider::Vllm)
            .map(|a| a.signals.iter().any(|s| s.rule_id == "vllm.header.server"))
            .unwrap_or(false);
        if vllm_distinct {
            if let Some(openai) = scores.get_mut(&AiProvider::OpenAi) {
                openai.raw_weight *= 0.6;
            }
        }
    }

    // LiteLLM proxy headers strongly indicate proxy layer
    if scores.contains_key(&AiProvider::LiteLlm) {
        let litellm_strong = scores
            .get(&AiProvider::LiteLlm)
            .map(|a| a.raw_weight >= 0.40)
            .unwrap_or(false);
        if litellm_strong {
            for other in [AiProvider::OpenAi, AiProvider::Vllm] {
                if let Some(acc) = scores.get_mut(&other) {
                    acc.raw_weight *= 0.7;
                }
            }
        }
    }
}

pub fn build_provider_fingerprint(
    provider: AiProvider,
    accumulator: ScoreAccumulator,
) -> ProviderFingerprint {
    let confidence = compute_confidence(&accumulator);
    ProviderFingerprint {
        provider,
        confidence,
        signals: accumulator.signals,
        inferred_api_style: infer_api_style(provider),
        suggested_method: suggest_method(provider),
    }
}

fn infer_api_style(provider: AiProvider) -> ApiStyle {
    match provider {
        AiProvider::OpenAi | AiProvider::AzureOpenAi | AiProvider::Vllm | AiProvider::LiteLlm => {
            ApiStyle::OpenAiCompatible
        }
        AiProvider::Anthropic => ApiStyle::AnthropicMessages,
        AiProvider::Gemini => ApiStyle::GeminiGenerateContent,
        AiProvider::Bedrock => ApiStyle::BedrockInvoke,
        AiProvider::Ollama => ApiStyle::OllamaNative,
    }
}

fn suggest_method(provider: AiProvider) -> Option<String> {
    match provider {
        AiProvider::Ollama => Some("POST".into()),
        AiProvider::Anthropic => Some("POST".into()),
        AiProvider::Bedrock => Some("POST".into()),
        AiProvider::Gemini => Some("POST".into()),
        _ => Some("POST".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_increases_with_weight() {
        let low = ScoreAccumulator {
            raw_weight: 0.25,
            signals: vec![MatchedSignal {
                provider: AiProvider::OpenAi,
                rule_id: "openai.path.chat".into(),
                description: "test".into(),
                weight: 0.25,
            }],
        };
        let high = ScoreAccumulator {
            raw_weight: 0.85,
            signals: vec![
                MatchedSignal {
                    provider: AiProvider::OpenAi,
                    rule_id: "openai.host".into(),
                    description: "host".into(),
                    weight: 0.35,
                },
                MatchedSignal {
                    provider: AiProvider::OpenAi,
                    rule_id: "openai.header.org".into(),
                    description: "header".into(),
                    weight: 0.40,
                },
            ],
        };
        assert!(compute_confidence(&high) > compute_confidence(&low));
    }

    #[test]
    fn single_strong_signal_floor() {
        let acc = ScoreAccumulator {
            raw_weight: 0.40,
            signals: vec![MatchedSignal {
                provider: AiProvider::Anthropic,
                rule_id: "anthropic.header.version".into(),
                description: "header".into(),
                weight: 0.40,
            }],
        };
        assert!(compute_confidence(&acc) >= 0.72);
    }

    #[test]
    fn azure_host_penalizes_openai() {
        let mut scores = HashMap::new();
        scores.insert(
            AiProvider::AzureOpenAi,
            ScoreAccumulator {
                raw_weight: 0.70,
                signals: vec![MatchedSignal {
                    provider: AiProvider::AzureOpenAi,
                    rule_id: "azure.host".into(),
                    description: "host".into(),
                    weight: 0.35,
                }],
            },
        );
        scores.insert(
            AiProvider::OpenAi,
            ScoreAccumulator {
                raw_weight: 0.55,
                signals: vec![MatchedSignal {
                    provider: AiProvider::OpenAi,
                    rule_id: "openai.path.chat".into(),
                    description: "path".into(),
                    weight: 0.30,
                }],
            },
        );

        apply_conflict_penalties(&mut scores);
        let openai = scores.get(&AiProvider::OpenAi).unwrap();
        assert!(openai.raw_weight < 0.55);
    }
}
