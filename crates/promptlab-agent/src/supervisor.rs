//! Yazg supervisor — ReAct routing to specialist sub-agents.

use std::collections::HashMap;
use std::sync::Arc;

use promptlab_judge::{JudgeEngine, JudgeRequest};
use promptlab_planner::PlannerLlm;
use promptlab_target_profile::{AttackResultsSummary, TargetProfile, VerificationResult, WizardAttackPlan};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::analyze_endpoint::{AnalyzeEndpointAgent, AnalyzeEndpointAgentOutcome};
use crate::attack_execution::{
    AgenticAttackExecutionAgent, AttackExecutionLlms, AttackExecutionOutcome, AttackExecutionRequest,
    AttackExecutionTools,
};
use crate::attack_plan::{AttackPlanAgent, AttackPlanAgentOutcome};
use crate::create_project::{CreateProjectTools, CreatedProject};
use crate::error::AgentResult;
use crate::generate_prompt::{GeneratePromptAgentOutcome, TechniquePromptContext};
use crate::judge_coordinator::JudgeCoordinatorAgentOutcome;
use crate::list_workspace::WorkspaceTools;
use crate::memory::{AgentMemoryStore, MemoryContext};
use crate::artifacts::{YazgActionKind, YazgArtifacts};
use crate::recommend::{RecommendAgent, RecommendAgentOutcome};
use crate::rig_runtime::{run_yazg_rig, YazgRigRequest};
use crate::rig_tools::{YazgRigLlms, YazgSpecialistContext};
use crate::sequential_attack_execution::{
    SequentialAttackExecutionAgent, SequentialAttackExecutionRequest,
};
use crate::summary::{SummaryAgent, SummaryAgentOutcome, SummaryRequest};
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
    CreateProject,
    ListWorkspace,
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
            "create_project" | "create-project" | "new_project" | "add_project" => {
                Self::CreateProject
            }
            "list_workspace"
            | "list-workspace"
            | "workspace"
            | "inventory"
            | "list_projects" => Self::ListWorkspace,
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
    /// Raw Rig `PromptResponse` (or equivalent engine payload) for UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_output: Option<serde_json::Value>,
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
    CreatedProject {
        turn: YazgTurn,
        project: CreatedProject,
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
            SupervisorIntent::CreateProject => {
                "User preference hint: create a workspace project via create_project \
                 (include JSON name; target is NOT required)."
            }
            SupervisorIntent::ListWorkspace => {
                "User preference hint: call list_workspace to read projects/targets/scans/findings \
                 from the local DB (including finding counts for a named project). \
                 Do not call analyze_endpoint. Do not invent rows."
            }
            SupervisorIntent::ExecuteAttack => {
                "User preference hint: agentic scan execution via AgenticAttackExecutionAgent may be appropriate."
            }
            SupervisorIntent::Chat => "User preference hint: conversational reply may be enough.",
            SupervisorIntent::Auto => {
                "No forced action — read the user prompt carefully, use STM/LTM for prior facts, \
                 call specialist tools only when needed, then finish with a clear reply."
            }
        };
        format!("{}\n\n({hint})", message.trim())
    }

    /// Primary entry: Rig agent loop (tool-calling) then map to typed delegation.
    pub async fn react_rig(request: YazgRigRequest) -> AgentResult<YazgDelegation> {
        let (artifacts, raw_output) = run_yazg_rig(request).await?;
        Ok(delegation_from_artifacts_with_raw(artifacts, Some(raw_output)))
    }

    /// Chat / command turn — Rig agent loop (no hard-coded agent branch).
    pub async fn handle(
        message: &str,
        intent_hint: SupervisorIntent,
        profile: Option<&TargetProfile>,
        auth_headers: HashMap<String, String>,
        llms: YazgRigLlms,
        memory: Option<Arc<dyn AgentMemoryStore>>,
        memory_ctx: MemoryContext,
        project_tools: Option<Arc<dyn CreateProjectTools>>,
        workspace_tools: Option<Arc<dyn WorkspaceTools>>,
    ) -> AgentResult<YazgDelegation> {
        let goal = Self::build_goal(message, intent_hint);
        let mut specialist = YazgSpecialistContext::default().with_auth(auth_headers);
        if let Some(p) = profile {
            specialist = specialist.with_profile(p.clone());
        }
        let request = YazgRigRequest::new(goal, llms)
            .with_memory(memory, memory_ctx)
            .with_project_tools(project_tools)
            .with_workspace_tools(workspace_tools)
            .with_specialist(specialist);
        Self::react_rig(request).await
    }

    /// Wizard Verification AI step: probe already captured.
    pub async fn react_classify_probe(
        profile: &TargetProfile,
        http: &promptlab_target_profile::VerifyHttpSuccess,
        llms: &YazgRigLlms,
        memory: Option<Arc<dyn AgentMemoryStore>>,
        memory_ctx: MemoryContext,
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
        let specialist = YazgSpecialistContext::default()
            .with_profile(profile.clone())
            .with_capability_probe(http.clone());
        let request = YazgRigRequest::new(goal, llms.clone())
            .with_memory(memory, memory_ctx)
            .with_specialist(specialist)
            .with_max_turns(4);
        let delegation = Self::react_rig(request).await?;
        if matches!(delegation, YazgDelegation::AnalyzedEndpoint { .. }) {
            return Ok(delegation);
        }

        warn!(
            endpoint = %profile.full_url(),
            "Yazg Rig missed AnalyzeEndpointAgent during Verification; forcing classify_probe"
        );
        let outcome =
            AnalyzeEndpointAgent::classify_probe(profile, http, llms.analyze.as_ref()).await?;
        let mut events = vec![AgentEvent::info(
            AgentId::Yazg,
            "Rig recovery: forced AnalyzeEndpointAgent after misrouted Verification turn",
        )];
        events.extend(outcome.events.clone());
        Ok(YazgDelegation::AnalyzedEndpoint {
            turn: YazgTurn {
                reply: format_analyze_success(&outcome.verification),
                intent: SupervisorIntent::AnalyzeEndpoint,
                events,
                verified: Some(true),
                plan_summary: None,
                raw_output: None,
            },
            outcome,
        })
    }

    /// Wizard attack-plan step.
    pub async fn react_plan(
        profile: &TargetProfile,
        llms: &YazgRigLlms,
        memory: Option<Arc<dyn AgentMemoryStore>>,
        memory_ctx: MemoryContext,
    ) -> AgentResult<YazgDelegation> {
        let message = format!(
            "Scan wizard Attack Plan: generate a FRESH attack plan for the verified target {}.\n\
             You MUST call action \"attack_plan\" in this turn even if short-term or long-term memory \
             mentions a prior plan — memory is historical context only, not a substitute for regenerating.\n\
             After AttackPlanAgent succeeds, finish with a short summary for the UI.\n\
             Do not finish early claiming a plan already exists.",
            profile.full_url()
        );
        let goal = Self::build_goal(&message, SupervisorIntent::AttackPlan);
        let specialist = YazgSpecialistContext::default().with_profile(profile.clone());
        let request = YazgRigRequest::new(goal, llms.clone())
            .with_memory(memory, memory_ctx)
            .with_specialist(specialist)
            .with_max_turns(4);
        let delegation = Self::react_rig(request).await?;
        if matches!(delegation, YazgDelegation::Planned { .. }) {
            return Ok(delegation);
        }

        warn!(
            endpoint = %profile.full_url(),
            "Yazg Rig missed AttackPlanAgent during Attack Plan; forcing AttackPlanAgent::run"
        );
        let outcome = AttackPlanAgent::run(profile, llms.plan.as_ref()).await?;
        let mut events = vec![AgentEvent::info(
            AgentId::Yazg,
            "Rig recovery: forced AttackPlanAgent after misrouted Attack Plan turn",
        )];
        events.extend(outcome.events.clone());
        let plan_summary = format_plan_summary(&outcome.plan);
        Ok(YazgDelegation::Planned {
            turn: YazgTurn {
                reply: format!("Attack plan generated.\n{plan_summary}"),
                intent: SupervisorIntent::AttackPlan,
                events,
                verified: Some(true),
                plan_summary: Some(plan_summary),
                raw_output: None,
            },
            outcome,
        })
    }

    /// Attack Factory: invent a novel technique probe.
    pub async fn react_generate_prompt(
        technique: &TechniquePromptContext,
        llms: &YazgRigLlms,
        memory: Option<Arc<dyn AgentMemoryStore>>,
        memory_ctx: MemoryContext,
    ) -> AgentResult<YazgDelegation> {
        let message = format!(
            "Attack Factory: invent an improved adversarial factory prompt for technique \
             `{}` (id={}, category={}). No scan target is required — technique context is enough. \
             Call generate_prompt, then finish with a short summary for the UI.",
            technique.name, technique.id, technique.category_id
        );
        let goal = Self::build_goal(&message, SupervisorIntent::GeneratePrompt);
        let specialist = YazgSpecialistContext::default().with_technique(technique.clone());
        let request = YazgRigRequest::new(goal, llms.clone())
            .with_memory(memory, memory_ctx)
            .with_specialist(specialist)
            .with_max_turns(4);
        Self::react_rig(request).await
    }

    /// Post-scan recommendations.
    pub async fn react_recommend(
        results: &AttackResultsSummary,
        llms: &YazgRigLlms,
        memory: Option<Arc<dyn AgentMemoryStore>>,
        memory_ctx: MemoryContext,
    ) -> AgentResult<YazgDelegation> {
        let message = format!(
            "Scan results: produce FRESH prioritized remediation recommendations for this completed \
             attack scan (status={}, findings={}).\n\
             You MUST call action \"recommend\" in this turn even if memory mentions prior recommendations \
             — memory is historical context only.\n\
             After RecommendAgent succeeds, finish with a short UI summary.\n\
             Do not finish early claiming recommendations already exist.",
            results.scan_status, results.total_findings
        );
        let goal = Self::build_goal(&message, SupervisorIntent::Recommend);
        let specialist = YazgSpecialistContext::default().with_attack_results(results.clone());
        let request = YazgRigRequest::new(goal, llms.clone())
            .with_memory(memory, memory_ctx)
            .with_specialist(specialist)
            .with_max_turns(4);
        let delegation = Self::react_rig(request).await?;
        if matches!(delegation, YazgDelegation::Recommended { .. }) {
            return Ok(delegation);
        }

        warn!(
            findings = results.total_findings,
            "Yazg Rig missed RecommendAgent; forcing RecommendAgent::run"
        );
        let outcome = RecommendAgent::run(results, llms.recommend.as_ref()).await?;
        let mut events = vec![AgentEvent::info(
            AgentId::Yazg,
            "Rig recovery: forced RecommendAgent after misrouted recommendations turn",
        )];
        events.extend(outcome.events.clone());
        Ok(YazgDelegation::Recommended {
            turn: YazgTurn {
                reply: format!(
                    "Recommendations ready ({} items).",
                    outcome.bundle.recommendations.len()
                ),
                intent: SupervisorIntent::Recommend,
                events,
                verified: None,
                plan_summary: None,
                raw_output: None,
            },
            outcome,
        })
    }

    /// Project or scan posture summary.
    pub async fn react_summarize(
        summary_request: &SummaryRequest,
        llms: &YazgRigLlms,
        memory: Option<Arc<dyn AgentMemoryStore>>,
        memory_ctx: MemoryContext,
    ) -> AgentResult<YazgDelegation> {
        let message = match summary_request {
            SummaryRequest::Project { project_name, .. } => format!(
                "Project Summary: produce a FRESH security-posture summary for project `{project_name}`.\n\
                 You MUST call action \"summary\" in this turn even if short-term or long-term memory \
                 mentions a prior summary — memory is historical context only, not a substitute.\n\
                 After SummaryAgent succeeds, finish with a short UI summary.\n\
                 Do not finish early claiming a summary already exists."
            ),
            SummaryRequest::Scan { summary } => format!(
                "Scan Summary: produce a FRESH summary for this attack scan (status={}, findings={}).\n\
                 You MUST call action \"summary\" in this turn even if memory mentions a prior summary \
                 — memory is historical context only.\n\
                 After SummaryAgent succeeds, finish with a short UI summary.\n\
                 Do not finish early claiming a summary already exists.",
                summary.scan_status, summary.total_findings
            ),
        };
        let goal = Self::build_goal(&message, SupervisorIntent::Summary);
        let specialist =
            YazgSpecialistContext::default().with_summary_request(summary_request.clone());
        let request = YazgRigRequest::new(goal, llms.clone())
            .with_memory(memory, memory_ctx)
            .with_specialist(specialist)
            .with_max_turns(4);
        let delegation = Self::react_rig(request).await?;
        if matches!(delegation, YazgDelegation::Summarized { .. }) {
            return Ok(delegation);
        }

        warn!(
            kind = %summary_request.kind_label(),
            "Yazg Rig missed SummaryAgent; forcing SummaryAgent::run"
        );
        let outcome = SummaryAgent::run(summary_request, llms.summary.as_ref()).await?;
        let mut events = vec![AgentEvent::info(
            AgentId::Yazg,
            "Rig recovery: forced SummaryAgent after misrouted summary turn",
        )];
        events.extend(outcome.events.clone());
        Ok(YazgDelegation::Summarized {
            turn: YazgTurn {
                reply: format!(
                    "{} summary ready ({} highlights).",
                    outcome.kind,
                    outcome.bundle.highlights.len()
                ),
                intent: SupervisorIntent::Summary,
                events,
                verified: None,
                plan_summary: None,
                raw_output: None,
            },
            outcome,
        })
    }

    /// Consensus judge a single probe via JudgeCoordinatorAgent workers.
    pub async fn react_judge(
        judge_request: &JudgeRequest,
        judge_engine: Arc<JudgeEngine>,
        llms: &YazgRigLlms,
        memory: Option<Arc<dyn AgentMemoryStore>>,
        memory_ctx: MemoryContext,
    ) -> AgentResult<YazgDelegation> {
        let message = format!(
            "Judge probe `{}` (category={}): run JudgeCoordinatorAgent so JudgeWorker, \
             ClassifierWorker, and AttackerWorker vote, then finish with the consensus verdict \
             for the UI. Probe response context is enough — no live target probe required.",
            judge_request.probe_id, judge_request.attack_category
        );
        let goal = Self::build_goal(&message, SupervisorIntent::Judge);
        let specialist =
            YazgSpecialistContext::default().with_judge(judge_request.clone(), judge_engine);
        let request = YazgRigRequest::new(goal, llms.clone())
            .with_memory(memory, memory_ctx)
            .with_specialist(specialist)
            .with_max_turns(4);
        Self::react_rig(request).await
    }

    /// Hard-gate agentic scan execution: Yazg delegates to AgenticAttackExecutionAgent.
    /// Host supplies HTTP/generate tools; ReflectionAgent + AttackPlanAgent::adapt run inside.
    pub async fn execute_attack(
        request: &AttackExecutionRequest,
        tools: &dyn AttackExecutionTools,
        llms: &AttackExecutionLlms,
        memory: Option<&dyn AgentMemoryStore>,
        memory_ctx: MemoryContext,
    ) -> AgentResult<AttackExecutionOutcome> {
        info!(
            category = %request.category,
            max_attempts = request.max_attempts,
            "Yazg delegating to AgenticAttackExecutionAgent"
        );
        let mut events = vec![AgentEvent::info(
            AgentId::Yazg,
            format!(
                "ExecuteAttack → AgenticAttackExecutionAgent ({})",
                request.category
            ),
        )];
        let mut outcome =
            AgenticAttackExecutionAgent::run(request, tools, llms, memory, memory_ctx).await?;
        events.append(&mut outcome.events);
        outcome.events = events;
        Ok(outcome)
    }

    /// Hard-gate sequential scan execution: Yazg delegates to SequentialAttackExecutionAgent.
    /// Generate → attack(+judge); ReAct recover on endpoint failures (no reflection/adapt).
    pub async fn execute_sequential_attack(
        request: &SequentialAttackExecutionRequest,
        tools: &dyn AttackExecutionTools,
        orchestrator: Option<Arc<dyn PlannerLlm>>,
        llm_ready: bool,
        memory: Option<&dyn AgentMemoryStore>,
        memory_ctx: MemoryContext,
    ) -> AgentResult<AttackExecutionOutcome> {
        info!(
            category = %request.category,
            "Yazg delegating to SequentialAttackExecutionAgent"
        );
        let mut events = vec![AgentEvent::info(
            AgentId::Yazg,
            format!(
                "ExecuteSequentialAttack → SequentialAttackExecutionAgent ({})",
                request.category
            ),
        )];
        let mut outcome = SequentialAttackExecutionAgent::run(
            request,
            tools,
            orchestrator,
            llm_ready,
            memory,
            memory_ctx,
        )
        .await?;
        events.append(&mut outcome.events);
        outcome.events = events;
        Ok(outcome)
    }

    /// Offline fallback when AI Runtime is unavailable (no Rig loop).
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
                "I am Yazg (offline). Start AI Runtime so I can read your prompt, use memory, and call tools.\n\n{target_line}"
            )
        } else {
            format!(
                "AI Runtime is not ready, so I cannot run the Rig agent loop.\n\n{target_line}"
            )
        };

        YazgTurn {
            reply,
            intent: SupervisorIntent::Chat,
            events: vec![AgentEvent::info(AgentId::Yazg, "Offline chat (no Rig)")],
            verified: profile.map(|p| p.is_verified()),
            plan_summary: None,
            raw_output: None,
        }
    }
}

fn delegation_from_artifacts_with_raw(
    artifacts: YazgArtifacts,
    raw_output: Option<serde_json::Value>,
) -> YazgDelegation {
    let intent = match artifacts.last_action {
        Some(YazgActionKind::AnalyzeEndpoint) => SupervisorIntent::AnalyzeEndpoint,
        Some(YazgActionKind::AttackPlan) => SupervisorIntent::AttackPlan,
        Some(YazgActionKind::GeneratePrompt) => SupervisorIntent::GeneratePrompt,
        Some(YazgActionKind::Recommend) => SupervisorIntent::Recommend,
        Some(YazgActionKind::Summary) => SupervisorIntent::Summary,
        Some(YazgActionKind::Judge) => SupervisorIntent::Judge,
        Some(YazgActionKind::CreateProject) => SupervisorIntent::CreateProject,
        Some(YazgActionKind::ListWorkspace) => SupervisorIntent::ListWorkspace,
        Some(YazgActionKind::Finish) | None => {
            if artifacts.created_project.is_some() {
                SupervisorIntent::CreateProject
            } else if artifacts.workspace_inventory.is_some() {
                SupervisorIntent::ListWorkspace
            } else if artifacts.judge.is_some() {
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
        raw_output,
    };

    if let Some(project) = artifacts.created_project.clone() {
        let mut turn = turn;
        turn.intent = SupervisorIntent::CreateProject;
        if turn.reply.trim().is_empty() {
            turn.reply = format!(
                "Created project \"{}\" (id={}).",
                project.name, project.id
            );
        }
        return YazgDelegation::CreatedProject { turn, project };
    }

    if let Some(outcome) = artifacts.judge {
        if matches!(intent, SupervisorIntent::Judge)
            || artifacts.last_action == Some(YazgActionKind::Judge)
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
            || artifacts.last_action == Some(YazgActionKind::Summary)
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
            || artifacts.last_action == Some(YazgActionKind::Recommend)
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
            || artifacts.last_action == Some(YazgActionKind::GeneratePrompt)
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
            || artifacts.last_action == Some(YazgActionKind::AttackPlan)
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
        promptlab_target_profile::ExecutionStrategy::Sequential => "sequential",
        promptlab_target_profile::ExecutionStrategy::Agentic => "agentic",
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
        assert_eq!(
            SupervisorIntent::parse("create_project"),
            SupervisorIntent::CreateProject
        );
        assert_eq!(
            SupervisorIntent::parse("new_project"),
            SupervisorIntent::CreateProject
        );
        assert_eq!(
            SupervisorIntent::parse("list_workspace"),
            SupervisorIntent::ListWorkspace
        );
        assert_eq!(
            SupervisorIntent::parse("inventory"),
            SupervisorIntent::ListWorkspace
        );
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
