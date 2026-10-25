//! Yazg supervisor — ReAct routing to specialist sub-agents.

use std::collections::HashMap;
use std::sync::Arc;

use promptlab_judge::{JudgeEngine, JudgeRequest};
use promptlab_planner::PlannerLlm;
use promptlab_target_profile::{AttackResultsSummary, TargetProfile, VerificationResult, WizardAttackPlan};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::analyze_endpoint::{AnalyzeEndpointAgent, AnalyzeEndpointAgentOutcome};
use crate::attack_execution::{
    AgenticAttackExecutionAgent, AttackExecutionLlms, AttackExecutionOutcome, AttackExecutionRequest,
    AttackExecutionTools,
};
use crate::attack_plan::{AttackPlanAgent, AttackPlanAgentOutcome};
use crate::create_project::{CreateProjectTools, CreatedProject};
use crate::error::AgentResult;
use crate::generate_prompt::{
    GeneratePromptAgent, GeneratePromptAgentOutcome, TechniquePromptContext,
};
use crate::judge_coordinator::{JudgeCoordinatorAgent, JudgeCoordinatorAgentOutcome};
use crate::list_workspace::WorkspaceTools;
use crate::memory::{AgentMemoryStore, MemoryContext, remember_stm, StmRole, StmWrite};
use crate::assistant::AssistantCapability;
use crate::hilt::{build_hilt_followup_goal, HiltPendingAction};
use crate::artifacts::{YazgActionKind, YazgArtifacts};
use crate::recommend::{RecommendAgent, RecommendAgentOutcome};
use crate::yazg_runtime::{run_yazg, YazgRequest};
use crate::yazg_tools::{YazgLlms, YazgSpecialistContext};
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
    /// Actual tool / action that produced this reply. `intent` collapses every
    /// workspace tool into `list_workspace`, so the UI needs the real name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    pub events: Vec<AgentEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_summary: Option<String>,
    /// Raw agent `PromptResponse` (or equivalent engine payload) for UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_output: Option<serde_json::Value>,
    /// AgentTrace id for this turn (links chat bubble → Trace detail).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    /// Mutating tool awaiting human approval (HILT). Present instead of applying the write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_action: Option<crate::hilt::HiltPendingAction>,
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
    /// Build a goal string; intent is only a soft hint (kept short — context engineering:
    /// do not dump internal policy into the user utterance stored in STM).
    pub fn build_goal(message: &str, intent_hint: SupervisorIntent) -> String {
        let message = message.trim();
        let hint = match intent_hint {
            SupervisorIntent::AnalyzeEndpoint => {
                Some("Hint: endpoint analysis may fit.")
            }
            SupervisorIntent::AttackPlan => Some("Hint: attack planning may fit."),
            SupervisorIntent::GeneratePrompt => {
                Some("Hint: Attack Factory prompt generation may fit.")
            }
            SupervisorIntent::Recommend => {
                Some("Hint: post-scan remediation recommendations may fit.")
            }
            SupervisorIntent::Summary => Some("Hint: project/scan summary may fit."),
            SupervisorIntent::Judge => {
                Some("Hint: consensus judging may fit.")
            }
            SupervisorIntent::CreateProject => {
                Some("Hint: create_project if the user wants a new project (name required).")
            }
            SupervisorIntent::ListWorkspace => {
                Some(
                    "Hint: use scoped workspace tools (list_targets / project_detail / \
                     list_findings / …) — not invent rows; prefer the smallest tool.",
                )
            }
            SupervisorIntent::ExecuteAttack => {
                Some("Hint: ReAct to pick SequentialAttackExecutionAgent or AgenticAttackExecutionAgent.")
            }
            // Keep Auto as the raw user message — system preamble already covers routing.
            SupervisorIntent::Chat | SupervisorIntent::Auto => None,
        };
        match hint {
            Some(h) => format!("{message}\n\n({h})"),
            None => message.to_string(),
        }
    }

    /// Primary entry: Yazg tool-calling loop (tool-calling) then map to typed delegation.
    pub async fn run_yazg_turn(request: YazgRequest) -> AgentResult<YazgDelegation> {
        let (artifacts, raw_output) = run_yazg(request).await?;
        Ok(delegation_from_artifacts_with_raw(artifacts, Some(raw_output)))
    }

    /// Chat / command turn — Yazg tool-calling loop (no hard-coded agent branch).
    pub async fn handle(
        message: &str,
        intent_hint: SupervisorIntent,
        profile: Option<&TargetProfile>,
        auth_headers: HashMap<String, String>,
        llms: YazgLlms,
        memory: Option<Arc<dyn AgentMemoryStore>>,
        memory_ctx: MemoryContext,
        project_tools: Option<Arc<dyn CreateProjectTools>>,
        workspace_tools: Option<Arc<dyn WorkspaceTools>>,
        agent_trace: Option<promptlab_agenttrace::SharedAgentTrace>,
        model_label: Option<String>,
    ) -> AgentResult<YazgDelegation> {
        let goal = Self::build_goal(message, intent_hint);
        let mut specialist = YazgSpecialistContext::default().with_auth(auth_headers);
        if let Some(p) = profile {
            specialist = specialist.with_profile(p.clone());
        }
        let request = YazgRequest::new(goal, llms)
            .with_memory(memory, memory_ctx)
            .with_project_tools(project_tools)
            .with_workspace_tools(workspace_tools)
            .with_agent_trace(agent_trace)
            .with_model_label(model_label)
            .with_specialist(specialist);
        Self::run_yazg_turn(request).await
    }

    /// Post-HILT conversation turn: model replies after approve/deny/expire.
    pub async fn hilt_followup(
        pending: &HiltPendingAction,
        decision: &str,
        tool_observation: Option<&str>,
        llms: YazgLlms,
        memory: Option<Arc<dyn AgentMemoryStore>>,
        memory_ctx: MemoryContext,
        agent_trace: Option<promptlab_agenttrace::SharedAgentTrace>,
        model_label: Option<String>,
    ) -> AgentResult<YazgTurn> {
        if let Some(obs) = tool_observation {
            remember_stm(
                memory.as_deref(),
                &memory_ctx,
                StmWrite {
                    agent_id: AgentId::Yazg,
                    role: StmRole::Observation,
                    memory_key: Some(pending.tool.clone()),
                    content: obs.to_string(),
                    content_json: Some(serde_json::json!({
                        "event": "hilt_resolved",
                        "decision": decision,
                        "pending_id": pending.id,
                    })),
                    importance: 0.75,
                },
            )
            .await;
        } else {
            remember_stm(
                memory.as_deref(),
                &memory_ctx,
                StmWrite {
                    agent_id: AgentId::Yazg,
                    role: StmRole::Observation,
                    memory_key: Some(pending.tool.clone()),
                    content: format!("HILT {decision}: {}", pending.summary),
                    content_json: Some(serde_json::json!({
                        "event": "hilt_resolved",
                        "decision": decision,
                        "pending_id": pending.id,
                        "summary": pending.summary,
                    })),
                    importance: 0.75,
                },
            )
            .await;
        }

        let goal = build_hilt_followup_goal(pending, decision, tool_observation);
        let request = YazgRequest::new(goal, llms)
            .with_memory(memory, memory_ctx)
            .with_max_turns(4)
            .with_agent_trace(agent_trace)
            .with_model_label(model_label)
            .with_forced_capability(AssistantCapability::Conversation)
            .with_skip_user_stm(true);

        let delegation = Self::run_yazg_turn(request).await?;
        let mut turn = match delegation {
            YazgDelegation::Chat { turn } => turn,
            YazgDelegation::AnalyzedEndpoint { turn, .. }
            | YazgDelegation::Planned { turn, .. }
            | YazgDelegation::GeneratedPrompt { turn, .. }
            | YazgDelegation::Recommended { turn, .. }
            | YazgDelegation::Summarized { turn, .. }
            | YazgDelegation::Judged { turn, .. }
            | YazgDelegation::CreatedProject { turn, .. }
            | YazgDelegation::ExecutedAttack { turn, .. } => turn,
        };

        if decision == "approve" {
            turn.intent = match pending.tool.as_str() {
                "create_project" => SupervisorIntent::CreateProject,
                _ => SupervisorIntent::Chat,
            };
            turn.action = Some(pending.tool.clone());
        } else {
            turn.action = Some(pending.tool.clone());
        }

        Ok(turn)
    }

    /// Scan wizard verification calls AnalyzeEndpointAgent directly.
    pub async fn react_classify_probe(
        profile: &TargetProfile,
        http: &promptlab_target_profile::VerifyHttpSuccess,
        llms: &YazgLlms,
        _memory: Option<Arc<dyn AgentMemoryStore>>,
        _memory_ctx: MemoryContext,
    ) -> AgentResult<YazgDelegation> {
        let outcome =
            AnalyzeEndpointAgent::classify_probe(profile, http, llms.analyze.as_ref()).await?;
        let events = outcome.events.clone();
        Ok(YazgDelegation::AnalyzedEndpoint {
            turn: YazgTurn {
                reply: format_analyze_success(&outcome.verification),
                intent: SupervisorIntent::AnalyzeEndpoint,
                action: Some("analyze_endpoint".into()),
                events,
                verified: Some(true),
                plan_summary: None,
                raw_output: None,
                pending_action: None,
                trace_id: None,
            },
            outcome,
        })
    }

    /// Scan wizard Attack Plan calls AttackPlanAgent directly.
    pub async fn react_plan(
        profile: &TargetProfile,
        llms: &YazgLlms,
        _memory: Option<Arc<dyn AgentMemoryStore>>,
        _memory_ctx: MemoryContext,
    ) -> AgentResult<YazgDelegation> {
        let outcome = AttackPlanAgent::run(profile, llms.plan.as_ref()).await?;
        let events = outcome.events.clone();
        let plan_summary = format_plan_summary(&outcome.plan);
        Ok(YazgDelegation::Planned {
            turn: YazgTurn {
                reply: format!("Attack plan generated.\n{plan_summary}"),
                intent: SupervisorIntent::AttackPlan,
                action: Some("attack_plan".into()),
                events,
                verified: Some(true),
                plan_summary: Some(plan_summary),
                raw_output: None,
                pending_action: None,
                trace_id: None,
            },
            outcome,
        })
    }

    /// Attack Factory and Prompt Lab call GeneratePromptAgent directly.
    pub async fn react_generate_prompt(
        technique: &TechniquePromptContext,
        llms: &YazgLlms,
        _memory: Option<Arc<dyn AgentMemoryStore>>,
        _memory_ctx: MemoryContext,
    ) -> AgentResult<YazgDelegation> {
        let outcome = GeneratePromptAgent::run(technique, llms.prompt.as_ref()).await?;
        let events = outcome.events.clone();
        Ok(YazgDelegation::GeneratedPrompt {
            turn: YazgTurn {
                reply: format!(
                    "Factory prompt generated for {} ({} chars).",
                    outcome.technique_id,
                    outcome.content.chars().count()
                ),
                intent: SupervisorIntent::GeneratePrompt,
                action: Some("generate_prompt".into()),
                events,
                verified: None,
                plan_summary: None,
                raw_output: None,
                pending_action: None,
                trace_id: None,
            },
            outcome,
        })
    }

    /// Scan results call RecommendAgent directly.
    pub async fn react_recommend(
        results: &AttackResultsSummary,
        llms: &YazgLlms,
        _memory: Option<Arc<dyn AgentMemoryStore>>,
        _memory_ctx: MemoryContext,
    ) -> AgentResult<YazgDelegation> {
        let outcome = RecommendAgent::run(results, llms.recommend.as_ref()).await?;
        let events = outcome.events.clone();
        Ok(YazgDelegation::Recommended {
            turn: YazgTurn {
                reply: format!(
                    "Recommendations ready ({} items).",
                    outcome.bundle.recommendations.len()
                ),
                intent: SupervisorIntent::Recommend,
                action: Some("recommend".into()),
                events,
                verified: None,
                plan_summary: None,
                raw_output: None,
                pending_action: None,
                trace_id: None,
            },
            outcome,
        })
    }

    /// Project details call SummaryAgent directly.
    pub async fn react_summarize(
        summary_request: &SummaryRequest,
        llms: &YazgLlms,
        _memory: Option<Arc<dyn AgentMemoryStore>>,
        _memory_ctx: MemoryContext,
    ) -> AgentResult<YazgDelegation> {
        let outcome = SummaryAgent::run(summary_request, llms.summary.as_ref()).await?;
        let events = outcome.events.clone();
        Ok(YazgDelegation::Summarized {
            turn: YazgTurn {
                reply: format!(
                    "{} summary ready ({} highlights).",
                    outcome.kind,
                    outcome.bundle.highlights.len()
                ),
                intent: SupervisorIntent::Summary,
                action: Some("summary".into()),
                events,
                verified: None,
                plan_summary: None,
                raw_output: None,
                pending_action: None,
                trace_id: None,
            },
            outcome,
        })
    }

    /// Judge a probe by calling JudgeCoordinatorAgent directly.
    pub async fn react_judge(
        judge_request: &JudgeRequest,
        judge_engine: Arc<JudgeEngine>,
        llms: &YazgLlms,
        _memory: Option<Arc<dyn AgentMemoryStore>>,
        _memory_ctx: MemoryContext,
    ) -> AgentResult<YazgDelegation> {
        let outcome = JudgeCoordinatorAgent::run(
            judge_request,
            judge_engine.as_ref(),
            Some(llms.judge.as_ref()),
        )
        .await?;
        let events = outcome.events.clone();
        let reply = format!(
            "Judge consensus: {} (confidence={:.0}%, votes={}).",
            outcome.verdict.verdict,
            outcome.verdict.confidence * 100.0,
            outcome.worker_results.len()
        );
        Ok(YazgDelegation::Judged {
            turn: YazgTurn {
                reply,
                intent: SupervisorIntent::Judge,
                action: Some("judge".into()),
                events,
                verified: None,
                plan_summary: None,
                raw_output: None,
                pending_action: None,
                trace_id: None,
            },
            outcome,
        })
    }

    /// Scan execution Agentic calls AgenticAttackExecutionAgent directly.
    /// ReflectionAgent and AttackPlanAgent::adapt run inside that agent.
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
            "AgenticAttackExecutionAgent run"
        );
        AgenticAttackExecutionAgent::run(request, tools, llms, memory, memory_ctx).await
    }

    /// Scan execution Sequential calls SequentialAttackExecutionAgent directly.
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
            "SequentialAttackExecutionAgent run"
        );
        SequentialAttackExecutionAgent::run(
            request,
            tools,
            orchestrator,
            llm_ready,
            memory,
            memory_ctx,
        )
        .await
    }

    /// Offline fallback when AI Runtime is unavailable (no Yazg loop).
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
                "AI Runtime is not ready, so I cannot run the Yazg agent loop.\n\n{target_line}"
            )
        };

        YazgTurn {
            reply,
            intent: SupervisorIntent::Chat,
                action: Some("chat".into()),
            events: vec![AgentEvent::info(AgentId::Yazg, "Offline chat (AI Runtime offline)")],
            verified: profile.map(|p| p.is_verified()),
            plan_summary: None,
            raw_output: None,
            pending_action: None,
            trace_id: None,
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
        Some(YazgActionKind::ListWorkspace)
        | Some(YazgActionKind::ProjectDetail)
        | Some(YazgActionKind::ListScan)
        | Some(YazgActionKind::ScanDetail)
        | Some(YazgActionKind::ListFindings)
        | Some(YazgActionKind::FindingDetail)
        | Some(YazgActionKind::ListTargets)
        | Some(YazgActionKind::TargetDetail)
        | Some(YazgActionKind::ListReports)
        | Some(YazgActionKind::ReportDetail) => SupervisorIntent::ListWorkspace,
        Some(YazgActionKind::Finish) | None => {
            if artifacts.pending_action.is_some() {
                match artifacts
                    .pending_action
                    .as_ref()
                    .map(|p| p.tool.as_str())
                {
                    Some("create_project") => SupervisorIntent::CreateProject,
                    _ => SupervisorIntent::Chat,
                }
            } else if artifacts.created_project.is_some() {
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
        action: artifacts
            .last_action
            .filter(|action| *action != YazgActionKind::Finish)
            .map(|action| action.as_str().to_string()),
        events: artifacts.events.clone(),
        verified: verified.or(artifacts.analyze.as_ref().map(|_| true)),
        plan_summary: plan_summary.clone(),
        raw_output,
        pending_action: None,
        trace_id: artifacts.trace_id.clone(),
    };

    if let Some(pending) = artifacts.pending_action.clone() {
        let mut turn = turn;
        turn.intent = match pending.tool.as_str() {
            "create_project" => SupervisorIntent::CreateProject,
            _ => turn.intent,
        };
        turn.action = Some(pending.tool.clone());
        if turn.reply.trim().is_empty() {
            turn.reply = pending.pending_user_reply();
        }
        turn.pending_action = Some(pending);
        return YazgDelegation::Chat { turn };
    }

    if let Some(project) = artifacts.created_project.clone() {
        let mut turn = turn;
        turn.intent = SupervisorIntent::CreateProject;
        turn.action = Some("create_project".into());
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
        assert!(goal.contains("Hint:"));
        assert!(goal.contains("attack planning"));
        assert!(!goal.to_ascii_lowercase().contains("prefer generating"));
        assert!(!goal.contains("AttackPlanAgent"));
    }

    #[test]
    fn auto_goal_keeps_raw_user_message() {
        let goal = YazgSupervisor::build_goal(
            "cho tôi các target trong project AI",
            SupervisorIntent::Auto,
        );
        assert_eq!(goal, "cho tôi các target trong project AI");
        assert!(!goal.contains("No forced action"));
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
