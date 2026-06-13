use std::collections::HashSet;

use crate::types::{
    AgentFramework, AiComponent, AttackRecommendation, DetectedComponent,
    DetectedFramework, DetectedTechnology, StackFingerprintReport,
};

/// Generate attack recommendations from a stack fingerprint report.
pub fn generate_attack_recommendations(report: &StackFingerprintReport) -> Vec<AttackRecommendation> {
    let mut recs: Vec<AttackRecommendation> = Vec::new();
    let mut seen = HashSet::new();

    let mut push = |category: &str, reason: &str, priority: u8| {
        if seen.insert(category.to_string()) {
            recs.push(AttackRecommendation {
                category: category.into(),
                reason: reason.into(),
                priority,
            });
        }
    };

    if report.provider_report.primary.is_some() || !report.technologies.is_empty() {
        push(
            "prompt_injection",
            "AI inference endpoint detected — test instruction override",
            1,
        );
        push(
            "jailbreak",
            "LLM deployment identified — test safety guardrail bypass",
            2,
        );
        push(
            "system_prompt_leakage",
            "Model API exposed — probe for system prompt extraction",
            3,
        );
        push(
            "data_exfiltration",
            "Generative API surface — test sensitive data extraction",
            4,
        );
    }

    for tech in &report.technologies {
        match tech.id.as_str() {
            "openai" | "azure_openai" | "openrouter" | "litellm" | "vllm" => {
                push(
                    "policy_bypass",
                    &format!("{} proxy/gateway — test content policy bypass", tech.name),
                    3,
                );
            }
            "anthropic" | "gemini" | "bedrock" => {
                push(
                    "policy_bypass",
                    &format!("{} API — test provider safety filter bypass", tech.name),
                    3,
                );
            }
            "ollama" => {
                push(
                    "policy_bypass",
                    "Local Ollama deployment — test uncensored local model abuse",
                    4,
                );
            }
            _ => {}
        }
    }

    for framework in &report.agent_frameworks {
        match framework.framework {
            AgentFramework::LangChain
            | AgentFramework::LangGraph
            | AgentFramework::LangServe
            | AgentFramework::CrewAi
            | AgentFramework::AutoGen => {
                push(
                    "tool_abuse",
                    &format!(
                        "{} agent framework — test tool/function call abuse",
                        framework.name
                    ),
                    1,
                );
            }
            AgentFramework::OpenWebUi
            | AgentFramework::AnythingLlm
            | AgentFramework::Flowise
            | AgentFramework::Dify => {
                push(
                    "rag_leakage",
                    &format!(
                        "{} platform — test knowledge base / RAG leakage",
                        framework.name
                    ),
                    2,
                );
                push(
                    "tool_abuse",
                    &format!("{} workflow UI — test plugin/tool abuse", framework.name),
                    3,
                );
            }
        }
    }

    for component in &report.ai_components {
        match component.component {
            AiComponent::McpServer => {
                push(
                    "mcp_abuse",
                    "MCP server detected — test tool/resource enumeration and abuse",
                    1,
                );
            }
            AiComponent::RagPipeline => {
                push(
                    "rag_leakage",
                    "RAG pipeline detected — test document/source leakage",
                    1,
                );
            }
            AiComponent::ToolOrchestration => {
                push(
                    "tool_abuse",
                    "Tool orchestration layer — test unauthorized tool invocation",
                    2,
                );
            }
        }
    }

    recs.sort_by_key(|r| r.priority);
    recs
}

pub fn technologies_from_providers(
    matches: &[crate::types::ProviderFingerprint],
) -> Vec<DetectedTechnology> {
    matches
        .iter()
        .map(|fp| DetectedTechnology {
            id: fp.provider.as_str().into(),
            name: fp.provider.display_name().into(),
            category: "inference_provider".into(),
            confidence: fp.confidence,
            signals: fp.signals.iter().map(|s| s.description.clone()).collect(),
        })
        .collect()
}

pub fn aggregate_confidence(
    technologies: &[DetectedTechnology],
    frameworks: &[DetectedFramework],
    components: &[DetectedComponent],
) -> f32 {
    let mut scores: Vec<f32> = technologies.iter().map(|t| t.confidence).collect();
    scores.extend(frameworks.iter().map(|f| f.confidence));
    scores.extend(components.iter().map(|c| c.confidence));
    scores
        .into_iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        ApiStyle, FingerprintReport, MatchedSignal, ProviderFingerprint, StackFingerprintReport,
    };
    use time::OffsetDateTime;

    #[test]
    fn langchain_recommends_tool_abuse() {
        let report = StackFingerprintReport {
            url: "https://example.com/invoke".into(),
            confidence: 0.8,
            technologies: vec![],
            agent_frameworks: vec![DetectedFramework {
                framework: AgentFramework::LangChain,
                name: "LangChain".into(),
                confidence: 0.75,
                signals: vec!["LangServe /invoke route".into()],
            }],
            ai_components: vec![],
            provider_report: FingerprintReport {
                url: "https://example.com/invoke".into(),
                matches: vec![],
                primary: None,
                analyzed_at: OffsetDateTime::now_utc(),
            },
            attack_recommendations: vec![],
            methods_used: vec![],
            analyzed_at: OffsetDateTime::now_utc(),
        };
        let recs = generate_attack_recommendations(&report);
        assert!(recs.iter().any(|r| r.category == "tool_abuse"));
    }

    #[test]
    fn mcp_recommends_mcp_abuse() {
        let report = StackFingerprintReport {
            url: "https://example.com/mcp".into(),
            confidence: 0.85,
            technologies: vec![],
            agent_frameworks: vec![],
            ai_components: vec![DetectedComponent {
                component: AiComponent::McpServer,
                name: "MCP Server".into(),
                confidence: 0.8,
                signals: vec!["MCP tools/list".into()],
            }],
            provider_report: FingerprintReport {
                url: "https://example.com/mcp".into(),
                matches: vec![],
                primary: None,
                analyzed_at: OffsetDateTime::now_utc(),
            },
            attack_recommendations: vec![],
            methods_used: vec![],
            analyzed_at: OffsetDateTime::now_utc(),
        };
        let recs = generate_attack_recommendations(&report);
        assert!(recs.iter().any(|r| r.category == "mcp_abuse"));
    }
}
