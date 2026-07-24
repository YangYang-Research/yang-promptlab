use std::collections::HashSet;

use promptlab_attack::AttackCategory;
use promptlab_planner::types::{AttackPlan, CategoryRationale, PlannerMode};

use crate::types::{TargetCapabilities, TargetProfile};

/// Generate an attack plan from Target Profile capabilities only.
/// No schema inference, no fingerprinting, no raw payload inspection.
pub fn plan_from_target_profile(profile: &TargetProfile) -> AttackPlan {
    let caps = crate::capabilities::effective_capabilities(profile);
    let mut plan = plan_from_capabilities(
        &caps,
        &profile.framework,
        profile.provider.as_str(),
    );
    plan.summary = summary_for_api_endpoint(&profile.full_url(), plan.categories.len());
    plan
}

pub fn summary_for_api_endpoint(api_endpoint: &str, category_count: usize) -> String {
    format!("Target profile plan for {api_endpoint}: {category_count} categories")
}

pub fn plan_from_capabilities(
    caps: &TargetCapabilities,
    framework: &str,
    provider: &str,
) -> AttackPlan {
    let mut rationales = Vec::new();
    let mut categories = Vec::new();
    let mut push = |category: AttackCategory, reason: String, priority: u8| {
        categories.push(category);
        rationales.push(CategoryRationale {
            category,
            reason,
            priority,
            source: "target_profile".into(),
        });
    };

    push(
        AttackCategory::PromptInjection,
        format!("AI endpoint ({provider}) — test prompt injection"),
        1,
    );
    push(
        AttackCategory::Jailbreak,
        "LLM deployment — test safety guardrail bypass".into(),
        2,
    );
    push(
        AttackCategory::SystemPromptExtraction,
        "Model API exposed — probe hidden instructions".into(),
        3,
    );

    if caps.supports_memory {
        push(
            AttackCategory::MemoryPoisoning,
            "Persistent memory enabled on target profile".into(),
            1,
        );
        push(
            AttackCategory::CrossUserLeakage,
            "Memory-enabled platform — test session isolation".into(),
            3,
        );
    }

    if caps.supports_tools || caps.supports_agent {
        push(
            AttackCategory::ToolAbuse,
            "Tool or agent capabilities declared on target profile".into(),
            1,
        );
        push(
            AttackCategory::AgentGoalHijacking,
            "Agent surface — test planner/goal manipulation".into(),
            2,
        );
    }

    if caps.supports_agent || framework == "mcp" || provider == "mcp" {
        push(
            AttackCategory::McpAbuse,
            "MCP or agent orchestration surface".into(),
            1,
        );
    }

    if caps.supports_conversation {
        push(
            AttackCategory::RagLeakage,
            "Conversation/RAG surface — test knowledge leakage".into(),
            2,
        );
    }

    let mut seen = HashSet::new();
    categories.retain(|c| seen.insert(*c));
    rationales.sort_by_key(|r| r.priority);

    let summary = format!(
        "Target profile plan for {provider} ({framework}): {} categories",
        categories.len()
    );

    AttackPlan {
        mode: PlannerMode::Deterministic,
        profile_id: "standard".into(),
        categories,
        disabled_tests: vec![],
        rationales,
        confidence: 0.85,
        summary,
        llm_rationale: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_uses_api_endpoint() {
        let profile = TargetProfile {
            base_url: "https://api.yyng.icu".into(),
            path: "/ycre/v1/code-review/github/completions".into(),
            ..Default::default()
        };
        let plan = plan_from_target_profile(&profile);
        assert_eq!(
            plan.summary,
            "Target profile plan for https://api.yyng.icu/ycre/v1/code-review/github/completions: 6 categories"
        );
    }
}
