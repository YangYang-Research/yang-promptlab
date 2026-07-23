//! Yazg supervisor — ReAct routing to specialist sub-agents.

use std::collections::HashMap;

use aisec_judge::{JudgeEngine, JudgeRequest};
use aisec_target_profile::{AttackResultsSummary, TargetProfile, VerificationResult, WizardAttackPlan};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::analyze_endpoint::{AnalyzeEndpointAgent, AnalyzeEndpointAgentOutcome};
use crate::attack_execution::{
    AttackExecutionAgent, AttackExecutionLlms, AttackExecutionOutcome, AttackExecutionRequest,
    AttackExecutionTools,
};
use crate::attack_plan::AttackPlanAgentOutcome;
use crate::error::AgentResult;
use crate::generate_prompt::{GeneratePromptAgentOutcome, TechniquePromptContext};
use crate::judge_coordinator::JudgeCoordinatorAgentOutcome;
use crate::react::{run_react, ReactActionKind, ReactArtifacts, ReactLlms, ReactRequest};
use crate::recommend::RecommendAgentOutcome;
use crate::summary::{SummaryAgentOutcome, SummaryRequest};
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
    GeneratePrompt,
    Recommend,
    Summary,
    Judge,
    ExecuteAttack,
}

impl SupervisorIntent {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "analyze_endpoint"
            | "analyze-endpoint"
            | "verify"
            | "verification" => Self::AnalyzeEndpoint,
            "plan" | "planner" | "attack_plan" | "attack-plan" => Self::AttackPlan,
            "generate_prompt"
            | "generate-prompt"
            | "factory_prompt"
            | "attack_factory" => Self::GeneratePrompt,
            "recommend" | "recommendation" | "recommendations" | "remediation" => {
                Self::Recommend
            }
            "summary" | "summarize" | "project_summary" | "scan_summary" => Self::Summary,
            "judge" | "judging" | "judge_coordinator" => Self::Judge,
            "execute_attack"
            | "execute-attack"
            | "attack_execution"
            | "agentic_scan"
            | "run_scan" => Self::ExecuteAttack,
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
    GeneratedPrompt {
        turn: YazgTurn,
        outcome: GeneratePromptAgentOutcome,
    },
    Recommended {
        turn: YazgTurn,
        outcome: RecommendAgentOutcome,
    },
    Summarized {
        turn: YazgTurn,
        outcome: SummaryAgentOutcome,
    },
    Judged {
        turn: YazgTurn,
        outcome: JudgeCoordinatorAgentOutcome,
    },
    ExecutedAttack {
        turn: YazgTurn,
        outcome: AttackExecutionOutcome,
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
            SupervisorIntent::GeneratePrompt => {
                "User preference hint: Attack Factory prompt generation may be appropriate."
            }
            SupervisorIntent::Recommend => {
                "User preference hint: post-scan remediation recommendations may be appropriate."
            }
            SupervisorIntent::Summary => {
                "User preference hint: project/scan summary may be appropriate."
            }
            SupervisorIntent::Judge => {
                "User preference hint: consensus judging via JudgeCoordinatorAgent may be appropriate."
            }
            SupervisorIntent::ExecuteAttack => {
                "User preference hint: agentic scan execution via AttackExecutionAgent may be appropriate."
            }
            SupervisorIntent::Chat => "User preference hint: conversational reply may be enough.",
            SupervisorIntent::Auto => "No forced action — choose freely via ReAct.",
        };
        format!("{}\n\n({hint})", message.trim())
    }

    /// Primary entry: ReAct reasoning then act on sub-agents.
    pub async fn react(
        request: ReactRequest<'_>,
        llms: &ReactLlms<'_>,
    ) -> AgentResult<YazgDelegation> {
        info!(goal = %truncate(&request.goal, 120), "Yazg ReAct handle");
        let artifacts = run_react(request, llms).await?;
        Ok(delegation_from_artifacts(artifacts))
    }

    /// Chat / command turn — always ReAct (no hard-coded agent branch).
    pub async fn handle(
        message: &str,
        intent_hint: SupervisorIntent,
        profile: Option<&TargetProfile>,
        auth_headers: HashMap<String, String>,
        llms: &ReactLlms<'_>,
    ) -> AgentResult<YazgDelegation> {
        let goal = Self::build_goal(message, intent_hint);
        let request = ReactRequest::new(goal)
            .with_profile(profile)
            .with_auth(auth_headers);
        Self::react(request, llms).await
    }

    /// Wizard Verification AI step: probe already captured.
    /// Soft intent hint only — Yazg ReAct chooses the agent/action.
    pub async fn react_classify_probe(
        profile: &TargetProfile,
        http: &aisec_target_profile::VerifyHttpSuccess,
        llms: &ReactLlms<'_>,
    ) -> AgentResult<YazgDelegation> {
        let message = format!(
            "Scan wizard Verification / endpoint analysis: a capability HTTP response was already \
             captured for {} (HTTP {}). Call analyze_endpoint to classify whether this is a live \
             generative AI API, then finish with a concise UI result. This is NOT Attack Factory — \
             do not call generate_prompt and do not ask for a technique.",
            profile.full_url(),
            http.status_code
        );
        let goal = Self::build_goal(&message, SupervisorIntent::AnalyzeEndpoint);
        let request = ReactRequest::new(goal)
            .with_profile(Some(profile))
            .with_capability_probe(Some(http))
            .with_max_steps(4);
        let delegation = Self::react(request, llms).await?;
        if matches!(delegation, YazgDelegation::AnalyzedEndpoint { .. }) {
            return Ok(delegation);
        }

        // Soft ReAct can misfire (e.g. Attack Factory "select a technique") on small models.
        // Wizard Verification always needs AnalyzeEndpointAgent — recover by calling it directly.
        warn!(
            endpoint = %profile.full_url(),
            "Yazg ReAct missed AnalyzeEndpointAgent during Verification; forcing classify_probe"
        );
        let outcome =
            AnalyzeEndpointAgent::classify_probe(profile, http, llms.analyze).await?;
        let mut events = vec![AgentEvent::info(
            AgentId::Yazg,
            "ReAct recovery: forced AnalyzeEndpointAgent after misrouted Verification turn",
        )];
        events.extend(outcome.events.clone());
        Ok(YazgDelegation::AnalyzedEndpoint {
            turn: YazgTurn {
                reply: format_analyze_success(&outcome.verification),
                intent: SupervisorIntent::AnalyzeEndpoint,
                events,
                verified: Some(true),
                plan_summary: None,
            },
            outcome,
        })
    }

    /// Wizard attack-plan step.
    /// Soft intent hint only — Yazg ReAct chooses the agent/action.
    pub async fn react_plan(
        profile: &TargetProfile,
        llms: &ReactLlms<'_>,
    ) -> AgentResult<YazgDelegation> {
        Self::handle(
            "Scan wizard Attack Plan: generate an attack plan for the current verified target, \
             then finish with a short summary for the UI.",
            SupervisorIntent::AttackPlan,
            Some(profile),
            HashMap::new(),
            llms,
        )
        .await
    }

    /// Attack Factory: invent a novel technique probe.
    /// Soft intent hint only — Yazg ReAct chooses the agent/action.
    pub async fn react_generate_prompt(
        technique: &TechniquePromptContext,
        llms: &ReactLlms<'_>,
    ) -> AgentResult<YazgDelegation> {
        let message = format!(
            "Attack Factory: invent an improved adversarial factory prompt for technique \
             `{}` (id={}, category={}). No scan target is required — technique context is enough. \
             Call generate_prompt, then finish with a short summary for the UI.",
            technique.name, technique.id, technique.category_id
        );
        let goal = Self::build_goal(&message, SupervisorIntent::GeneratePrompt);
        let request = ReactRequest::new(goal)
            .with_technique(Some(technique))
            .with_max_steps(4);
        Self::react(request, llms).await
    }

    /// Post-scan recommendations.
    /// Soft intent hint only — Yazg ReAct chooses the agent/action.
    pub async fn react_recommend(
        results: &AttackResultsSummary,
        llms: &ReactLlms<'_>,
    ) -> AgentResult<YazgDelegation> {
        let message = format!(
            "Scan results: produce prioritized remediation recommendations for this completed \
             attack scan (status={}, findings={}). No live target probe is required — attack \
             results context is enough. Call recommend, then finish with a short UI summary.",
            results.scan_status, results.total_findings
        );
        let goal = Self::build_goal(&message, SupervisorIntent::Recommend);
        let request = ReactRequest::new(goal)
            .with_attack_results(Some(results))
            .with_max_steps(4);
        Self::react(request, llms).await
    }

    /// Project or scan posture summary.
    /// Soft intent hint only — Yazg ReAct chooses the agent/action.
    pub async fn react_summarize(
        summary_request: &SummaryRequest,
        llms: &ReactLlms<'_>,
    ) -> AgentResult<YazgDelegation> {
        let message = match summary_request {
            SummaryRequest::Project { project_name, .. } => format!(
                "Project Summary: summarize overall security posture for project `{project_name}`. \
                 No live scan target is required — project assessment context is enough. \
                 Call summary, then finish with a short UI summary."
            ),
            SummaryRequest::Scan { summary } => format!(
                "Scan Summary: summarize this attack scan (status={}, findings={}). \
                 No live target probe is required — scan results context is enough. \
                 Call summary, then finish with a short UI summary.",
                summary.scan_status, summary.total_findings
            ),
        };
        let goal = Self::build_goal(&message, SupervisorIntent::Summary);
        let request = ReactRequest::new(goal)
            .with_summary_request(Some(summary_request))
            .with_max_steps(4);
        Self::react(request, llms).await
    }

    /// Consensus judge a single probe via JudgeCoordinatorAgent workers.
    /// Soft intent hint only — Yazg ReAct chooses the agent/action.
    pub async fn react_judge(
        judge_request: &JudgeRequest,
        judge_engine: &JudgeEngine,
        llms: &ReactLlms<'_>,
    ) -> AgentResult<YazgDelegation> {
        let message = format!(
            "Judge probe `{}` (category={}): run JudgeCoordinatorAgent so JudgeWorker, \
             ClassifierWorker, and AttackerWorker vote, then finish with the consensus verdict \
             for the UI. Probe response context is enough — no live target probe required.",
            judge_request.probe_id, judge_request.attack_category
        );
        let goal = Self::build_goal(&message, SupervisorIntent::Judge);
        let request = ReactRequest::new(goal)
            .with_judge(Some(judge_request), Some(judge_engine))
            .with_max_steps(4);
        Self::react(request, llms).await
    }

    /// Hard-gate agentic scan execution: Yazg delegates to AttackExecutionAgent.
    /// Host supplies HTTP/generate tools; ReflectionAgent + AttackPlanAgent::adapt run inside.
    pub async fn execute_attack(
        request: &AttackExecutionRequest,
        tools: &dyn AttackExecutionTools,
        llms: &AttackExecutionLlms<'_>,
    ) -> AgentResult<AttackExecutionOutcome> {
        info!(
            category = %request.category,
            max_attempts = request.max_attempts,
            "Yazg delegating to AttackExecutionAgent"
        );
        let mut events = vec![AgentEvent::info(
            AgentId::Yazg,
            format!(
                "ExecuteAttack → AttackExecutionAgent ({})",
                request.category
            ),
        )];
        let mut outcome = AttackExecutionAgent::run(request, tools, llms).await?;
        events.append(&mut outcome.events);
        outcome.events = events;
        Ok(outcome)
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
                 AnalyzeEndpointAgent / AttackPlanAgent / GeneratePromptAgent / \
                 RecommendAgent / SummaryAgent / JudgeCoordinatorAgent.\n\n{target_line}"
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
        Some(ReactActionKind::GeneratePrompt) => SupervisorIntent::GeneratePrompt,
        Some(ReactActionKind::Recommend) => SupervisorIntent::Recommend,
        Some(ReactActionKind::Summary) => SupervisorIntent::Summary,
        Some(ReactActionKind::Judge) => SupervisorIntent::Judge,
        Some(ReactActionKind::Finish) | None => {
            if artifacts.judge.is_some() {
                SupervisorIntent::Judge
            } else if artifacts.summary.is_some() {
                SupervisorIntent::Summary
            } else if artifacts.recommend.is_some() {
                SupervisorIntent::Recommend
            } else if artifacts.generate_prompt.is_some() {
                SupervisorIntent::GeneratePrompt
            } else if artifacts.plan.is_some() {
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
    let verified = artifacts.analyze.as_ref().map(|_| true);

    let turn = YazgTurn {
        reply: artifacts.final_reply.clone(),
        intent,
        events: artifacts.events.clone(),
        verified: verified.or(artifacts.analyze.as_ref().map(|_| true)),
        plan_summary: plan_summary.clone(),
    };

    if let Some(outcome) = artifacts.judge {
        if matches!(intent, SupervisorIntent::Judge)
            || artifacts.last_action == Some(ReactActionKind::Judge)
        {
            let mut turn = turn;
            turn.intent = SupervisorIntent::Judge;
            if turn.reply.trim().is_empty() {
                turn.reply = format!(
                    "Judge consensus: {} (confidence={:.0}%, votes={}).",
                    outcome.verdict.verdict,
                    outcome.verdict.confidence * 100.0,
                    outcome.worker_results.len()
                );
            }
            return YazgDelegation::Judged { turn, outcome };
        }
    }

    if let Some(outcome) = artifacts.summary {
        if matches!(intent, SupervisorIntent::Summary)
            || artifacts.last_action == Some(ReactActionKind::Summary)
        {
            let mut turn = turn;
            turn.intent = SupervisorIntent::Summary;
            if turn.reply.trim().is_empty() {
                turn.reply = format!(
                    "{} summary ready ({} highlights).",
                    outcome.kind,
                    outcome.bundle.highlights.len()
                );
            }
            return YazgDelegation::Summarized { turn, outcome };
        }
    }

    if let Some(outcome) = artifacts.recommend {
        if matches!(intent, SupervisorIntent::Recommend)
            || artifacts.last_action == Some(ReactActionKind::Recommend)
        {
            let mut turn = turn;
            turn.intent = SupervisorIntent::Recommend;
            if turn.reply.trim().is_empty() {
                turn.reply = format!(
                    "Recommendations ready ({} items).",
                    outcome.bundle.recommendations.len()
                );
            }
            return YazgDelegation::Recommended { turn, outcome };
        }
    }

    if let Some(outcome) = artifacts.generate_prompt {
        if matches!(intent, SupervisorIntent::GeneratePrompt)
            || artifacts.last_action == Some(ReactActionKind::GeneratePrompt)
        {
            let mut turn = turn;
            turn.intent = SupervisorIntent::GeneratePrompt;
            if turn.reply.trim().is_empty() {
                turn.reply = format!(
                    "Factory prompt generated for {} ({} chars).",
                    outcome.technique_id,
                    outcome.content.chars().count()
                );
            }
            return YazgDelegation::GeneratedPrompt { turn, outcome };
        }
    }

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
    fn parses_generate_prompt_intent_aliases() {
        assert_eq!(
            SupervisorIntent::parse("generate_prompt"),
            SupervisorIntent::GeneratePrompt
        );
        assert_eq!(
            SupervisorIntent::parse("attack_factory"),
            SupervisorIntent::GeneratePrompt
        );
    }

    #[test]
    fn parses_recommend_and_summary_intents() {
        assert_eq!(
            SupervisorIntent::parse("recommend"),
            SupervisorIntent::Recommend
        );
        assert_eq!(
            SupervisorIntent::parse("project_summary"),
            SupervisorIntent::Summary
        );
        assert_eq!(SupervisorIntent::parse("judge"), SupervisorIntent::Judge);
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
