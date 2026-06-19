use crate::deterministic::plan_deterministic;
use crate::error::PlannerResult;
use crate::local_llm::{plan_with_local_llm, PlannerLlm};
use crate::types::{AttackPlan, FingerprintResult, PlannerMode};

/// Generate an attack plan from fingerprint observations.
pub async fn generate_attack_plan(
    input: &FingerprintResult,
    mode: PlannerMode,
    llm: Option<&dyn PlannerLlm>,
) -> PlannerResult<AttackPlan> {
    match mode {
        PlannerMode::Deterministic => Ok(plan_deterministic(input)),
        PlannerMode::LocalLlm => {
            let backend = llm.ok_or_else(|| {
                crate::error::PlannerError::InvalidInput(
                    "local LLM planner requires a configured vault model".into(),
                )
            })?;
            plan_with_local_llm(input, backend).await
        }
    }
}
