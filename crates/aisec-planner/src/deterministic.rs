use std::collections::HashMap;

use aisec_attack::AttackCategory;
use aisec_fingerprint::generate_attack_recommendations;

use crate::normalize::normalize_fingerprint_category;
use crate::types::{AttackPlan, CategoryRationale, FingerprintResult, MergedCapabilities, PlannerMode};

pub fn plan_deterministic(input: &FingerprintResult) -> AttackPlan {
    let caps = input.merged_capabilities();
    let mut rationales: HashMap<AttackCategory, CategoryRationale> = HashMap::new();

    let mut push = |category: AttackCategory, reason: String, priority: u8, source: &str| {
        rationales
            .entry(category)
            .and_modify(|existing| {
                if priority < existing.priority {
                    existing.priority = priority;
                    existing.reason = reason.clone();
                    existing.source = source.into();
                }
            })
            .or_insert(CategoryRationale {
                category,
                reason,
                priority,
                source: source.into(),
            });
    };

    if caps.has_ai_surface {
        push(
            AttackCategory::PromptInjection,
            "AI inference or agent surface detected".into(),
            1,
            "capability_rule",
        );
        push(
            AttackCategory::Jailbreak,
            "LLM deployment identified — test safety guardrail bypass".into(),
            2,
            "capability_rule",
        );
        push(
            AttackCategory::SystemPromptExtraction,
            "Model API or chat UI exposed — probe hidden instructions".into(),
            3,
            "capability_rule",
        );
    }

    if caps.memory_enabled {
        push(
            AttackCategory::MemoryPoisoning,
            "Persistent memory or conversation history enabled".into(),
            1,
            "capability_rule",
        );
        push(
            AttackCategory::CrossUserLeakage,
            "Memory-enabled platform — test session isolation".into(),
            3,
            "capability_rule",
        );
    }

    if caps.tools_enabled {
        push(
            AttackCategory::ToolAbuse,
            "Tool orchestration or function calling enabled".into(),
            1,
            "capability_rule",
        );
        push(
            AttackCategory::AgentGoalHijacking,
            "Agent with tools — test planner/goal manipulation".into(),
            2,
            "capability_rule",
        );
    }

    if caps.rag_enabled {
        push(
            AttackCategory::RagLeakage,
            "RAG or knowledge-base retrieval enabled".into(),
            1,
            "capability_rule",
        );
    }

    if caps.mcp_detected || caps.platforms.contains("mcp_server") {
        push(
            AttackCategory::McpAbuse,
            "MCP server detected — test tool/resource abuse".into(),
            1,
            "capability_rule",
        );
    }

    for platform in &caps.platforms {
        apply_platform_rules(platform, &mut push);
    }

    for endpoint in &input.endpoints {
        for rec in generate_attack_recommendations(&endpoint.report) {
            if let Some(category) = normalize_fingerprint_category(&rec.category) {
                push(category, rec.reason, rec.priority, "fingerprint_rule");
            }
        }
    }

    let mut ordered: Vec<CategoryRationale> = rationales.into_values().collect();
    ordered.sort_by_key(|r| (r.priority, r.category.as_str().to_string()));

    let categories: Vec<AttackCategory> = ordered.iter().map(|r| r.category).collect();
    let profile_id = infer_profile_id(&categories);
    let summary = build_summary(&caps, &categories);

    AttackPlan {
        mode: PlannerMode::Deterministic,
        profile_id,
        categories,
        disabled_tests: Vec::new(),
        rationales: ordered,
        confidence: infer_confidence(input, &caps),
        summary,
        llm_rationale: None,
    }
}

fn apply_platform_rules<F>(platform: &str, push: &mut F)
where
    F: FnMut(AttackCategory, String, u8, &str),
{
    match platform {
        "openwebui" | "librechat" | "dify" | "flowise" | "langflow" | "anythingllm" => {
            push(
                AttackCategory::RagLeakage,
                format!("{platform} workflow UI — knowledge/RAG leakage"),
                2,
                "platform_rule",
            );
            push(
                AttackCategory::ToolAbuse,
                format!("{platform} platform — plugin/tool abuse"),
                2,
                "platform_rule",
            );
        }
        "openai_api" | "azure_openai_api" | "openrouter_api" | "litellm_api" | "vllm_api" => {
            push(
                AttackCategory::Jailbreak,
                "OpenAI-compatible gateway — policy bypass".into(),
                3,
                "platform_rule",
            );
        }
        "anthropic_api" | "gemini_api" | "bedrock_api" => {
            push(
                AttackCategory::Jailbreak,
                "Hosted model API — provider safety filter bypass".into(),
                3,
                "platform_rule",
            );
        }
        "ollama_api" => {
            push(
                AttackCategory::Jailbreak,
                "Local Ollama — uncensored local model abuse".into(),
                4,
                "platform_rule",
            );
        }
        "langchain" | "langgraph" | "langserve" | "crewai" | "autogen" => {
            push(
                AttackCategory::AgentGoalHijacking,
                format!("{platform} agent framework — goal hijacking"),
                2,
                "platform_rule",
            );
        }
        _ => {}
    }
}

fn infer_profile_id(categories: &[AttackCategory]) -> String {
    if categories.is_empty() {
        return "quick".into();
    }
    if categories.len() <= 3 {
        "quick".into()
    } else if categories.len() <= 6 {
        "standard".into()
    } else {
        "deep".into()
    }
}

fn build_summary(caps: &MergedCapabilities, categories: &[AttackCategory]) -> String {
    let mut parts = Vec::new();
    if !caps.platforms.is_empty() {
        parts.push(caps.platforms.iter().cloned().collect::<Vec<_>>().join(", "));
    }
    let flags: Vec<&str> = [
        caps.memory_enabled.then_some("memory"),
        caps.tools_enabled.then_some("tools"),
        caps.rag_enabled.then_some("rag"),
        caps.mcp_detected.then_some("mcp"),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !flags.is_empty() {
        parts.push(format!("capabilities: {}", flags.join("+")));
    }
    if categories.is_empty() {
        return "No attack categories selected".into();
    }
    let cat_names: Vec<_> = categories.iter().map(|c| c.display_name()).collect();
    if parts.is_empty() {
        format!("Suggested attacks: {}", cat_names.join(", "))
    } else {
        format!(
            "{} => {}",
            parts.join(" · "),
            cat_names.join(", ")
        )
    }
}

fn infer_confidence(input: &FingerprintResult, caps: &MergedCapabilities) -> f32 {
    let mut scores: Vec<f32> = input
        .endpoints
        .iter()
        .map(|e| e.report.confidence)
        .filter(|s| *s > 0.0)
        .collect();
    if caps.has_ai_surface && scores.is_empty() {
        scores.push(0.55);
    }
    scores.into_iter().fold(0.0_f32, f32::max).min(1.0)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use aisec_fingerprint::{
        AgentFramework, DetectedFramework, FingerprintReport, PlatformProfile, StackFingerprintReport,
    };
    use time::OffsetDateTime;

    fn openwebui_with_tools_memory() -> FingerprintResult {
        FingerprintResult::single(
            "ep-1",
            "https://target.example/api/v1/chats",
            StackFingerprintReport {
                url: "https://target.example/api/v1/chats".into(),
                confidence: 0.82,
                technologies: vec![],
                agent_frameworks: vec![DetectedFramework {
                    framework: AgentFramework::OpenWebUi,
                    name: "OpenWebUI".into(),
                    confidence: 0.8,
                    signals: vec!["OpenWebUI chats API".into()],
                }],
                ai_components: vec![],
                provider_report: FingerprintReport {
                    url: String::new(),
                    matches: vec![],
                    primary: None,
                    analyzed_at: OffsetDateTime::now_utc(),
                },
                attack_recommendations: vec![],
                methods_used: vec![],
                platform_profile: PlatformProfile {
                    platform: "openwebui".into(),
                    version: String::new(),
                    auth_type: "bearer".into(),
                    llm_provider: "openai".into(),
                    memory_enabled: true,
                    tools_enabled: true,
                    rag_enabled: false,
                },
                analyzed_at: OffsetDateTime::now_utc(),
            },
        )
    }

    #[test]
    fn openwebui_tools_memory_plans_expected_categories() {
        let plan = plan_deterministic(&openwebui_with_tools_memory());
        let ids: HashSet<_> = plan.categories.iter().map(|c| c.as_str()).collect();
        assert!(ids.contains("prompt_injection"));
        assert!(ids.contains("tool_abuse"));
        assert!(ids.contains("memory_poisoning"));
    }
}
