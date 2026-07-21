//! Yazg supervisor — ReAct routing to AnalyzeEndpointAgent / AttackPlanAgent.

use std::collections::HashMap;

use aisec_planner::PlannerLlm;
use aisec_target_profile::{TargetProfile, VerificationResult, WizardAttackPlan};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::analyze_endpoint::AnalyzeEndpointAgentOutcome;
use crate::attack_plan::AttackPlanAgentOutcome;
use crate::error::AgentResult;
use crate::react::{run_react, ReactActionKind, ReactArtifacts, ReactRequest};
use crate::types::{AgentEvent, AgentId};

/// Soft hint for the ReAct goal (not a hard route).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SupervisorIntent {
    #[default]
    Auto,
    Chat,
    AnalyzeEndpoint,
    AttackPlan,
}

impl SupervisorIntent {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "analyze_endpoint"
            | "analyze-endpoint"
            | "verify"
            | "verification" => Self::AnalyzeEndpoint,
            "plan" | "planner" | "attack_plan" | "attack-plan" => Self::AttackPlan,
            "chat" | "help" => Self::Chat,
            "auto" | "" => Self::Auto,
            _ => Self::Auto,
        }
    }
}

/// One supervisor turn returned to the host / chat UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YazgTurn {
    pub reply: String,
    pub intent: SupervisorIntent,
    pub events: Vec<AgentEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_summary: Option<String>,
}

/// Host-facing payloads when the caller needs typed domain results.
#[derive(Debug)]
pub enum YazgDelegation {
    Chat { turn: YazgTurn },
    AnalyzedEndpoint {
        turn: YazgTurn,
        outcome: AnalyzeEndpointAgentOutcome,
    },
    Planned {
        turn: YazgTurn,
        outcome: AttackPlanAgentOutcome,
    },
}

/// Yazg — top-level supervisor agent (ReAct).
pub struct YazgSupervisor;

impl YazgSupervisor {
    /// Build a goal string; intent is only a soft hint inside the goal.
    pub fn build_goal(message: &str, intent_hint: SupervisorIntent) -> String {
        let hint = match intent_hint {
            SupervisorIntent::AnalyzeEndpoint => {
                "User preference hint: endpoint analysis may be appropriate."
            }
            SupervisorIntent::AttackPlan => {
                "User preference hint: attack planning may be appropriate."
            }
            SupervisorIntent::Chat => "User preference hint: conversational reply may be enough.",
            SupervisorIntent::Auto => "No forced action — choose freely via ReAct.",
        };
        format!("{}\n\n({hint})", message.trim())
    }

    /// Primary entry: ReAct reasoning then act on sub-agents.
    pub async fn react(
        request: ReactRequest<'_>,
        supervisor_llm: &dyn PlannerLlm,
        analyze_llm: &dyn PlannerLlm,
        plan_llm: &dyn PlannerLlm,
    ) -> AgentResult<YazgDelegation> {
        info!(goal = %truncate(&request.goal, 120), "Yazg ReAct handle");
        let artifacts = run_react(request, supervisor_llm, analyze_llm, plan_llm).await?;
        Ok(delegation_from_artifacts(artifacts))
    }

    /// Chat / command turn — always ReAct (no hard-coded agent branch).
    pub async fn handle(
        message: &str,
        intent_hint: SupervisorIntent,
        profile: Option<&TargetProfile>,
        auth_headers: HashMap<String, String>,
        supervisor_llm: &dyn PlannerLlm,
        analyze_llm: &dyn PlannerLlm,
        plan_llm: &dyn PlannerLlm,
    ) -> AgentResult<YazgDelegation> {
        let goal = Self::build_goal(message, intent_hint);
        let request = ReactRequest::new(goal)
            .with_profile(profile)
            .with_auth(auth_headers);
        Self::react(request, supervisor_llm, analyze_llm, plan_llm).await
    }

    /// Wizard Verification AI step: probe already captured.
    /// Soft intent hint only — Yazg ReAct chooses the agent/action.
    pub async fn react_classify_probe(
        profile: &TargetProfile,
        http: &aisec_target_profile::VerifyHttpSuccess,
        supervisor_llm: &dyn PlannerLlm,
        analyze_llm: &dyn PlannerLlm,
        plan_llm: &dyn PlannerLlm,
    ) -> AgentResult<YazgDelegation> {
        let message = format!(
            "Scan wizard Verification: a capability probe already succeeded for {} (HTTP {}). \
             Classify whether this endpoint is a live generative AI API, then finish with a concise UI result.",
            profile.full_url(),
            http.status_code
        );
        let goal = Self::build_goal(&message, SupervisorIntent::AnalyzeEndpoint);
        let request = ReactRequest::new(goal)
            .with_profile(Some(profile))
            .with_capability_probe(Some(http))
            .with_max_steps(4);
        Self::react(request, supervisor_llm, analyze_llm, plan_llm).await
    }

    /// Wizard attack-plan step.
    /// Soft intent hint only — Yazg ReAct chooses the agent/action.
    pub async fn react_plan(
        profile: &TargetProfile,
        supervisor_llm: &dyn PlannerLlm,
        analyze_llm: &dyn PlannerLlm,
        plan_llm: &dyn PlannerLlm,
    ) -> AgentResult<YazgDelegation> {
        Self::handle(
            "Scan wizard Attack Plan: generate an attack plan for the current verified target, \
             then finish with a short summary for the UI.",
            SupervisorIntent::AttackPlan,
            Some(profile),
            HashMap::new(),
            supervisor_llm,
            analyze_llm,
            plan_llm,
        )
        .await
    }

    /// Offline fallback when AI Runtime is unavailable (no ReAct).
    pub fn offline_chat(message: &str, profile: Option<&TargetProfile>) -> YazgTurn {
        let trimmed = message.trim();
        let target_line = match profile {
            Some(p) => format!(
                "Active target: `{}` ({}) — verified={}.",
                p.full_url(),
                p.provider.as_str(),
                p.is_verified()
            ),
            None => "No target selected.".into(),
        };

        let reply = if trimmed.is_empty()
            || matches!(
                trimmed.to_ascii_lowercase().as_str(),
                "help" | "?" | "hi" | "hello"
            ) {
            format!(
                "I am Yazg (offline). Start AI Runtime so I can ReAct-route to \
                 AnalyzeEndpointAgent / AttackPlanAgent.\n\n{target_line}"
            )
        } else {
            format!(
                "AI Runtime is not ready, so I cannot run the ReAct loop.\n\n{target_line}"
            )
        };

        YazgTurn {
            reply,
            intent: SupervisorIntent::Chat,
            events: vec![AgentEvent::info(AgentId::Yazg, "Offline chat (no ReAct)")],
            verified: profile.map(|p| p.is_verified()),
            plan_summary: None,
        }
    }
}

fn delegation_from_artifacts(artifacts: ReactArtifacts) -> YazgDelegation {
    let intent = match artifacts.last_action {
        Some(ReactActionKind::AnalyzeEndpoint) => SupervisorIntent::AnalyzeEndpoint,
        Some(ReactActionKind::AttackPlan) => SupervisorIntent::AttackPlan,
        Some(ReactActionKind::Finish) | None => {
            if artifacts.plan.is_some() {
                SupervisorIntent::AttackPlan
            } else if artifacts.analyze.is_some() {
                SupervisorIntent::AnalyzeEndpoint
            } else {
                SupervisorIntent::Chat
            }
        }
    };

    let plan_summary = artifacts
        .plan
        .as_ref()
        .map(|p| format_plan_summary(&p.plan));
    let verified = artifacts
        .analyze
        .as_ref()
        .map(|_| true)
        .or_else(|| None);

    let turn = YazgTurn {
        reply: artifacts.final_reply.clone(),
        intent,
        events: artifacts.events.clone(),
        verified: verified.or(artifacts.analyze.as_ref().map(|_| true)),
        plan_summary: plan_summary.clone(),
    };

    if let Some(outcome) = artifacts.plan {
        if matches!(intent, SupervisorIntent::AttackPlan)
            || artifacts.last_action == Some(ReactActionKind::AttackPlan)
        {
            let mut turn = turn;
            turn.intent = SupervisorIntent::AttackPlan;
            turn.plan_summary = Some(format_plan_summary(&outcome.plan));
            turn.verified = Some(true);
            if turn.reply.trim().is_empty() {
                turn.reply = format!(
                    "Attack plan generated.\n{}",
                    turn.plan_summary.clone().unwrap_or_default()
                );
            }
            return YazgDelegation::Planned { turn, outcome };
        }
    }

    if let Some(outcome) = artifacts.analyze {
        let mut turn = turn;
        turn.intent = SupervisorIntent::AnalyzeEndpoint;
        turn.verified = Some(true);
        if turn.reply.trim().is_empty() {
            turn.reply = format_analyze_success(&outcome.verification);
        }
        return YazgDelegation::AnalyzedEndpoint { turn, outcome };
    }

    YazgDelegation::Chat { turn }
}

fn format_analyze_success(v: &VerificationResult) -> String {
    format!(
        "AnalyzeEndpointAgent confirmed an AI endpoint.\n\
         Status: {} · HTTP {} · {} ms\n\
         Provider: {} · Model: {}",
        v.status,
        v.status_code,
        v.response_time_ms,
        v.provider,
        v.model.as_deref().unwrap_or("unknown")
    )
}

fn format_plan_summary(plan: &WizardAttackPlan) -> String {
    let cats: Vec<&str> = plan.categories.iter().map(|c| c.as_str()).collect();
    let execution = match plan.execution_strategy {
        aisec_target_profile::ExecutionStrategy::Sequential => "sequential",
        aisec_target_profile::ExecutionStrategy::Agentic => "agentic",
    };
    format!(
        "Profile: {} · Categories: {} · Modes: {} · Execution: {} · Source: {}",
        plan.profile_id,
        cats.join(", "),
        plan.profile_modes.len(),
        execution,
        plan.planner_source
    )
}

fn truncate(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    let shortened: String = t.chars().take(max.saturating_sub(1)).collect();
    format!("{shortened}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goal_includes_soft_hint() {
        let goal = YazgSupervisor::build_goal("check the api", SupervisorIntent::AnalyzeEndpoint);
        assert!(goal.contains("check the api"));
        assert!(goal.contains("endpoint analysis"));
    }

    #[test]
    fn attack_plan_goal_is_soft_hint_not_hard_route() {
        let goal = YazgSupervisor::build_goal(
            "generate an attack plan",
            SupervisorIntent::AttackPlan,
        );
        assert!(goal.contains("generate an attack plan"));
        assert!(goal.contains("User preference hint"));
        assert!(!goal.to_ascii_lowercase().contains("prefer generating"));
        assert!(!goal.contains("AttackPlanAgent"));
    }

    #[test]
    fn parses_attack_plan_intent_aliases() {
        assert_eq!(SupervisorIntent::parse("plan"), SupervisorIntent::AttackPlan);
        assert_eq!(
            SupervisorIntent::parse("attack_plan"),
            SupervisorIntent::AttackPlan
        );
    }
}
