//! AttackPlanAgent — sub-agent for wizard attack-plan generation.
//!
//! Kind **A**: LLM planning over a verified target profile (shared runtime).

use aisec_planner::PlannerLlm;
use aisec_target_profile::{build_wizard_attack_plan_with_llm, TargetProfile, WizardAttackPlan};
use tracing::info;

use crate::error::{AgentError, AgentResult};
use crate::types::{AgentEvent, AgentId};

/// Outcome of an AttackPlanAgent run.
#[derive(Debug, Clone)]
pub struct AttackPlanAgentOutcome {
    pub plan: WizardAttackPlan,
    pub events: Vec<AgentEvent>,
}

/// Attack-plan sub-agent under Yazg.
pub struct AttackPlanAgent;

impl AttackPlanAgent {
    /// Generate (or refine) a wizard attack plan from a verified profile.
    pub async fn run(
        profile: &TargetProfile,
        llm: &dyn PlannerLlm,
    ) -> AgentResult<AttackPlanAgentOutcome> {
        if !profile.is_verified() {
            return Err(AgentError::InvalidInput(
                "AttackPlanAgent requires a verified target profile".into(),
            ));
        }

        let mut events = vec![AgentEvent::started(
            AgentId::AttackPlan,
            format!("Planning attacks for {}", profile.full_url()),
        )];

        info!(endpoint = %profile.full_url(), "AttackPlanAgent started");

        match build_wizard_attack_plan_with_llm(profile, llm).await {
            Ok(plan) => {
                events.push(AgentEvent::completed(
                    AgentId::AttackPlan,
                    format!(
                        "Attack plan ready ({} categories, {} modes, source={})",
                        plan.categories.len(),
                        plan.profile_modes.len(),
                        plan.planner_source
                    ),
                ));
                Ok(AttackPlanAgentOutcome { plan, events })
            }
            Err(err) => {
                let message = err.to_string();
                events.push(AgentEvent::failed(AgentId::AttackPlan, message.clone()));
                Err(AgentError::AttackPlan(message))
            }
        }
    }
}
