use aisec_attack::AttackCategory;
use aisec_planner::{generate_attack_plan, AttackPlan, FingerprintResult, PlannerLlm};

use crate::error::{AgentError, AgentResult};
use crate::types::AgentConfig;

/// Build an attack plan from fingerprint observations.
pub async fn plan_attacks(
    config: &AgentConfig,
    fingerprint: &FingerprintResult,
    llm: Option<&dyn PlannerLlm>,
) -> AgentResult<AttackPlan> {
    let plan = generate_attack_plan(fingerprint, config.planner_mode, llm).await?;
    if plan.categories.is_empty() {
        return Err(AgentError::InvalidInput(
            "planner produced no attack categories".into(),
        ));
    }
    Ok(plan)
}

/// Restrict planner categories to the scan playbook selection.
pub fn intersect_categories(plan: &AttackPlan, allowed: &[AttackCategory]) -> AttackPlan {
    let allowed_set: std::collections::HashSet<_> = allowed.iter().copied().collect();
    let categories: Vec<_> = plan
        .categories
        .iter()
        .copied()
        .filter(|c| allowed_set.contains(c))
        .collect();
    let fallback = if categories.is_empty() {
        allowed.to_vec()
    } else {
        categories
    };
    AttackPlan {
        categories: fallback,
        ..plan.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aisec_planner::PlannerMode;

    #[test]
    fn intersect_keeps_allowed_categories() {
        let plan = AttackPlan {
            mode: PlannerMode::Deterministic,
            profile_id: "custom".into(),
            categories: vec![
                AttackCategory::PromptInjection,
                AttackCategory::ToolAbuse,
                AttackCategory::McpAbuse,
            ],
            disabled_tests: vec![],
            rationales: vec![],
            confidence: 0.9,
            summary: String::new(),
            llm_rationale: None,
        };
        let allowed = vec![
            AttackCategory::PromptInjection,
            AttackCategory::MemoryPoisoning,
        ];
        let narrowed = intersect_categories(&plan, &allowed);
        assert_eq!(narrowed.categories, vec![AttackCategory::PromptInjection]);
    }
}
