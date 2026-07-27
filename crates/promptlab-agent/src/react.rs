//! Soft ReAct loop for Yazg: Thought → Action → Observation until `finish`.

use std::collections::HashMap;

use promptlab_judge::{JudgeEngine, JudgeRequest};
use promptlab_planner::PlannerLlm;
use promptlab_target_profile::{AttackResultsSummary, TargetProfile, VerifyHttpSuccess};
use serde::Deserialize;
use tracing::{info, warn};

use crate::agent_log::{log_llm_call, log_react, log_tool_call, AgentLogContext};
use crate::analyze_endpoint::{AnalyzeEndpointAgent, AnalyzeEndpointAgentOutcome};
use crate::attack_plan::{AttackPlanAgent, AttackPlanAgentOutcome};
use crate::create_project::{CreateProjectTools, CreatedProject};
use crate::error::{AgentError, AgentResult};
use crate::generate_prompt::{
    GeneratePromptAgent, GeneratePromptAgentOutcome, TechniquePromptContext,
};
use crate::judge_coordinator::{JudgeCoordinatorAgent, JudgeCoordinatorAgentOutcome};
use crate::list_workspace::{WorkspaceInventory, WorkspaceTools};
use crate::memory::{
    load_memory_prompt_block, remember_ltm, remember_stm, AgentMemoryStore, LtmWrite,
    MemoryContext, MemoryScopeType, StmRole, StmWrite,
};
use crate::recommend::{RecommendAgent, RecommendAgentOutcome};
use crate::summary::{SummaryAgent, SummaryAgentOutcome, SummaryRequest};
use crate::types::{AgentEvent, AgentId};

const DEFAULT_MAX_STEPS: usize = 6;
/// Invalid / non-JSON supervisor replies do not consume action steps; keep asking until valid.
const MAX_PARSE_FAILURES: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactActionKind {
    AnalyzeEndpoint,
    AttackPlan,
    GeneratePrompt,
    Recommend,
    Summary,
    Judge,
    CreateProject,
    ListWorkspace,
    Finish,
}

#[derive(Debug, Clone, Default)]
pub struct ReactArtifacts {
    pub events: Vec<AgentEvent>,
    pub analyze: Option<AnalyzeEndpointAgentOutcome>,
    pub plan: Option<AttackPlanAgentOutcome>,
    pub generate_prompt: Option<GeneratePromptAgentOutcome>,
    pub recommend: Option<RecommendAgentOutcome>,
    pub summary: Option<SummaryAgentOutcome>,
    pub judge: Option<JudgeCoordinatorAgentOutcome>,
    pub created_project: Option<CreatedProject>,
    pub workspace_inventory: Option<WorkspaceInventory>,
    pub final_reply: String,
    pub last_action: Option<ReactActionKind>,
}

/// LLM handles for each ReAct actor (supervisor + specialist sub-agents).
pub struct ReactLlms<'a> {
    pub supervisor: &'a dyn PlannerLlm,
    pub analyze: &'a dyn PlannerLlm,
    pub plan: &'a dyn PlannerLlm,
    pub prompt: &'a dyn PlannerLlm,
    pub recommend: &'a dyn PlannerLlm,
    pub summary: &'a dyn PlannerLlm,
}

#[derive(Clone)]
pub struct ReactRequest<'a> {
    pub goal: String,
    pub profile: Option<&'a TargetProfile>,
    pub auth_headers: HashMap<String, String>,
    /// When set, AnalyzeEndpointAgent classifies this probe (skips HTTP).
    pub capability_probe: Option<&'a VerifyHttpSuccess>,
    /// When set, GeneratePromptAgent can produce a factory probe.
    pub technique: Option<&'a TechniquePromptContext>,
    /// When set, RecommendAgent can produce post-scan recommendations.
    pub attack_results: Option<&'a AttackResultsSummary>,
    /// When set, SummaryAgent can produce project/scan summary.
    pub summary_request: Option<&'a SummaryRequest>,
    /// When set with `judge_engine`, JudgeCoordinatorAgent can score a probe.
    pub judge_request: Option<&'a JudgeRequest>,
    /// Runtime/role pool for JudgeCoordinatorAgent workers.
    pub judge_engine: Option<&'a JudgeEngine>,
    /// Host project creation tool (assistant chat CRUD).
    pub project_tools: Option<&'a dyn CreateProjectTools>,
    /// Host workspace inventory tool (projects / targets / scans / findings).
    pub workspace_tools: Option<&'a dyn WorkspaceTools>,
    /// Optional host memory store (STM/LTM).
    pub memory: Option<&'a dyn AgentMemoryStore>,
    /// Session / entity scope for memory ops.
    pub memory_ctx: MemoryContext,
    pub max_steps: usize,
}

impl<'a> ReactRequest<'a> {
    pub fn new(goal: impl Into<String>) -> Self {
        Self {
            goal: goal.into(),
            profile: None,
            auth_headers: HashMap::new(),
            capability_probe: None,
            technique: None,
            attack_results: None,
            summary_request: None,
            judge_request: None,
            judge_engine: None,
            project_tools: None,
            workspace_tools: None,
            memory: None,
            memory_ctx: MemoryContext::default(),
            max_steps: DEFAULT_MAX_STEPS,
        }
    }

    pub fn with_profile(mut self, profile: Option<&'a TargetProfile>) -> Self {
        self.profile = profile;
        self
    }

    pub fn with_auth(mut self, auth_headers: HashMap<String, String>) -> Self {
        self.auth_headers = auth_headers;
        self
    }

    pub fn with_capability_probe(mut self, probe: Option<&'a VerifyHttpSuccess>) -> Self {
        self.capability_probe = probe;
        self
    }

    pub fn with_technique(mut self, technique: Option<&'a TechniquePromptContext>) -> Self {
        self.technique = technique;
        self
    }

    pub fn with_attack_results(mut self, results: Option<&'a AttackResultsSummary>) -> Self {
        self.attack_results = results;
        self
    }

    pub fn with_summary_request(mut self, request: Option<&'a SummaryRequest>) -> Self {
        self.summary_request = request;
        self
    }

    pub fn with_judge(
        mut self,
        request: Option<&'a JudgeRequest>,
        engine: Option<&'a JudgeEngine>,
    ) -> Self {
        self.judge_request = request;
        self.judge_engine = engine;
        self
    }

    pub fn with_project_tools(mut self, tools: Option<&'a dyn CreateProjectTools>) -> Self {
        self.project_tools = tools;
        self
    }

    pub fn with_workspace_tools(mut self, tools: Option<&'a dyn WorkspaceTools>) -> Self {
        self.workspace_tools = tools;
        self
    }

    pub fn with_memory(
        mut self,
        store: Option<&'a dyn AgentMemoryStore>,
        ctx: MemoryContext,
    ) -> Self {
        self.memory = store;
        self.memory_ctx = ctx;
        self
    }

    pub fn with_max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps.max(1);
        self
    }

    fn log_ctx(&self) -> AgentLogContext<'_> {
        AgentLogContext::new()
            .with_project(self.memory_ctx.project_id.as_deref())
            .with_target(self.memory_ctx.target_id.as_deref())
            .with_scan(self.memory_ctx.scan_id.as_deref())
    }
}

#[derive(Debug, Deserialize)]
struct ReactStepJson {
    thought: Option<String>,
    action: String,
    #[serde(default)]
    reply: Option<String>,
    /// Project name when action is create_project.
    #[serde(default)]
    name: Option<String>,
    #[serde(default, alias = "project_name")]
    project_name: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

pub async fn run_react(
    request: ReactRequest<'_>,
    llms: &ReactLlms<'_>,
) -> AgentResult<ReactArtifacts> {
    let mut artifacts = ReactArtifacts::default();
    artifacts.events.push(AgentEvent::started(
        AgentId::Yazg,
        "ReAct loop started",
    ));

    remember_stm(
        request.memory,
        &request.memory_ctx,
        StmWrite {
            agent_id: AgentId::Yazg,
            role: StmRole::User,
            memory_key: Some("goal".into()),
            content: request.goal.clone(),
            content_json: None,
            importance: 0.7,
        },
    )
    .await;

    let memory_block =
        load_memory_prompt_block(request.memory, &request.memory_ctx, AgentId::Yazg, None).await;

    let mut transcript = String::new();
    transcript.push_str(
        "You are Yazg, the in-app AI Assistant.\n\
         Workflow each turn:\n\
         1) Read and analyze the user prompt.\n\
         2) Use short-term / long-term memory for facts already known about this workspace.\n\
         3) Call a specialist tool only when needed for fresh work \
         (analyze_endpoint, attack_plan, generate_prompt, recommend, summary, judge, \
         create_project, list_workspace).\n\
         4) When you have enough to answer, finish with a concise reply.\n\
         Do not invent tool results. Prefer finish for general app questions.\n\
         For create_project: include JSON fields name (required) and optional description; \
         do not ask for a scan target — projects do not need a target.\n\
         For questions about existing projects/targets/scans/findings, call list_workspace \
         first — do not invent DB inventory.\n\n",
    );
    transcript.push_str(&format!("User prompt:\n{}\n\n", request.goal.trim()));
    if !memory_block.is_empty() {
        transcript.push_str(&memory_block);
    }
    transcript.push_str(&format_context(
        request.profile,
        request.capability_probe.is_some(),
        request.technique,
        request.attack_results.is_some(),
        request.summary_request,
        request.judge_request.is_some() && request.judge_engine.is_some(),
    ));
    transcript.push_str(
        "\nBegin ReAct. Respond with one JSON step (thought + action).\n\
         Reply with a single JSON object only — no markdown fences, no prose outside JSON.\n\
         Example: {\"thought\":\"Plan attacks for the verified target\",\"action\":\"attack_plan\"}\n",
    );

    let mut step = 0usize;
    let mut parse_failures = 0usize;
    while step < request.max_steps {
        let display_step = step + 1;
        info!(step = display_step, "Yazg ReAct step");
        artifacts.events.push(AgentEvent::info(
            AgentId::Yazg,
            format!("ReAct step {display_step}/{max}", max = request.max_steps),
        ));

        let raw = match llms.supervisor.complete(&transcript).await {
            Ok(raw) => {
                log_llm_call(
                    AgentId::Yazg,
                    "supervisor",
                    transcript.len(),
                    &raw,
                    true,
                    request.log_ctx(),
                );
                raw
            }
            Err(err) => {
                log_llm_call(
                    AgentId::Yazg,
                    "supervisor",
                    transcript.len(),
                    &err.to_string(),
                    false,
                    request.log_ctx(),
                );
                return Err(AgentError::Supervisor(format!(
                    "ReAct reasoning failed: {err}"
                )));
            }
        };

        let parsed = match parse_react_step(&raw) {
            Ok(parsed) => parsed,
            Err(err) => {
                parse_failures = parse_failures.saturating_add(1);
                warn!(
                    error = %err,
                    parse_failures,
                    raw = %truncate(&raw, 400),
                    "Yazg ReAct parse failed; retrying"
                );
                log_react(
                    AgentId::Yazg,
                    "parse_retry",
                    format!("{err} ({parse_failures}/{MAX_PARSE_FAILURES})"),
                    request.log_ctx(),
                );
                artifacts.events.push(AgentEvent::info(
                    AgentId::Yazg,
                    format!(
                        "Invalid ReAct JSON ({parse_failures}/{MAX_PARSE_FAILURES}): {err} — retrying"
                    ),
                ));
                if parse_failures >= MAX_PARSE_FAILURES {
                    return Err(AgentError::Supervisor(format!(
                        "ReAct response did not contain a valid JSON object after {parse_failures} attempts: {err}"
                    )));
                }
                transcript.push_str(&format!(
                    "\nObservation: INVALID supervisor response ({err}).\n\
                     Raw preview: {}\n\
                     Respond again with ONE JSON object only, no markdown, no extra text.\n\
                     Required shape: {{\"thought\":\"...\",\"action\":\"analyze_endpoint|attack_plan|generate_prompt|recommend|summary|judge|create_project|list_workspace|finish\"}}\n\
                     For attack planning use action \"attack_plan\".\n\
                     For workspace/DB inventory use action \"list_workspace\".\n",
                    truncate(&raw, 240)
                ));
                continue;
            }
        };

        let thought = parsed
            .thought
            .unwrap_or_else(|| "(no thought)".into())
            .trim()
            .to_string();

        let action = match parse_action_kind(&parsed.action) {
            Ok(action) => action,
            Err(err) => {
                parse_failures = parse_failures.saturating_add(1);
                let message = err.to_string();
                warn!(
                    error = %message,
                    parse_failures,
                    action = %parsed.action,
                    "Yazg ReAct unknown action; retrying"
                );
                log_react(
                    AgentId::Yazg,
                    "action_retry",
                    format!("{message} ({parse_failures}/{MAX_PARSE_FAILURES})"),
                    request.log_ctx(),
                );
                artifacts.events.push(AgentEvent::info(
                    AgentId::Yazg,
                    format!(
                        "Invalid ReAct action ({parse_failures}/{MAX_PARSE_FAILURES}): {message} — retrying"
                    ),
                ));
                if parse_failures >= MAX_PARSE_FAILURES {
                    return Err(err);
                }
                transcript.push_str(&format!(
                    "\n--- Attempt ---\nThought: {thought}\nAction: {}\n\
                     Observation: INVALID action ({message}).\n\
                     Choose one of: analyze_endpoint, attack_plan, generate_prompt, recommend, \
                     summary, judge, create_project, list_workspace, finish.\n\
                     Reply with one JSON object only.\n",
                    parsed.action
                ));
                continue;
            }
        };

        step = step.saturating_add(1);
        artifacts.last_action = Some(action);
        artifacts.events.push(AgentEvent::react(
            AgentId::Yazg,
            format!("Thought: {thought}"),
        ));
        artifacts.events.push(AgentEvent::react(
            AgentId::Yazg,
            format!("Action: {}", parsed.action),
        ));

        transcript.push_str(&format!(
            "\n--- Step {step} ---\nThought: {thought}\nAction: {}\n",
            parsed.action
        ));

        match action {
            ReactActionKind::Finish => {
                let mut reply = parsed
                    .reply
                    .filter(|r| !r.trim().is_empty())
                    .unwrap_or_else(|| thought.clone());
                // Models often finish after list_workspace with a vague next-step suggestion.
                // Prefer the concrete DB inventory Observation for the user-visible reply.
                if let Some(inventory) = artifacts.workspace_inventory.as_ref() {
                    if !inventory.reply_covers_inventory(&reply) {
                        reply = inventory.to_user_reply();
                    }
                }
                artifacts.final_reply = reply.clone();
                log_tool_call(
                    AgentId::Yazg,
                    "finish",
                    truncate(&reply, 400),
                    request.log_ctx(),
                );
                remember_stm(
                    request.memory,
                    &request.memory_ctx,
                    StmWrite {
                        agent_id: AgentId::Yazg,
                        role: StmRole::Assistant,
                        memory_key: Some("reply".into()),
                        content: reply,
                        content_json: None,
                        importance: 0.6,
                    },
                )
                .await;
                persist_react_ltm(&request, &artifacts).await;
                artifacts.events.push(AgentEvent::completed(
                    AgentId::Yazg,
                    "ReAct finished",
                ));
                return Ok(artifacts);
            }
            ReactActionKind::AnalyzeEndpoint => {
                log_tool_call(
                    AgentId::Yazg,
                    "analyze_endpoint",
                    "delegating to AnalyzeEndpointAgent",
                    request.log_ctx(),
                );
                let observation =
                    execute_analyze(&request, llms.analyze, &mut artifacts).await?;
                push_observation(&mut transcript, &mut artifacts, &observation);
                remember_observation(&request, AgentId::AnalyzeEndpoint, &observation).await;
            }
            ReactActionKind::AttackPlan => {
                log_tool_call(
                    AgentId::Yazg,
                    "attack_plan",
                    "delegating to AttackPlanAgent",
                    request.log_ctx(),
                );
                let observation =
                    execute_attack_plan(&request, llms.plan, &mut artifacts).await?;
                push_observation(&mut transcript, &mut artifacts, &observation);
                remember_observation(&request, AgentId::AttackPlan, &observation).await;
            }
            ReactActionKind::GeneratePrompt => {
                log_tool_call(
                    AgentId::Yazg,
                    "generate_prompt",
                    "delegating to GeneratePromptAgent",
                    request.log_ctx(),
                );
                let observation =
                    execute_generate_prompt(&request, llms.prompt, &mut artifacts).await?;
                push_observation(&mut transcript, &mut artifacts, &observation);
                remember_observation(&request, AgentId::GeneratePrompt, &observation).await;
            }
            ReactActionKind::Recommend => {
                log_tool_call(
                    AgentId::Yazg,
                    "recommend",
                    "delegating to RecommendAgent",
                    request.log_ctx(),
                );
                let observation =
                    execute_recommend(&request, llms.recommend, &mut artifacts).await?;
                push_observation(&mut transcript, &mut artifacts, &observation);
                remember_observation(&request, AgentId::Recommend, &observation).await;
            }
            ReactActionKind::Summary => {
                log_tool_call(
                    AgentId::Yazg,
                    "summary",
                    "delegating to SummaryAgent",
                    request.log_ctx(),
                );
                let observation =
                    execute_summary(&request, llms.summary, &mut artifacts).await?;
                push_observation(&mut transcript, &mut artifacts, &observation);
                remember_observation(&request, AgentId::Summary, &observation).await;
            }
            ReactActionKind::Judge => {
                log_tool_call(
                    AgentId::Yazg,
                    "judge",
                    "delegating to JudgeCoordinatorAgent",
                    request.log_ctx(),
                );
                let observation = execute_judge(&request, &mut artifacts).await?;
                push_observation(&mut transcript, &mut artifacts, &observation);
                remember_observation(&request, AgentId::JudgeCoordinator, &observation).await;
            }
            ReactActionKind::CreateProject => {
                let name = parsed
                    .name
                    .or(parsed.project_name)
                    .unwrap_or_default();
                let description = parsed.description;
                log_tool_call(
                    AgentId::Yazg,
                    "create_project",
                    format!("name={name}"),
                    request.log_ctx(),
                );
                let observation = execute_create_project(
                    &request,
                    &name,
                    description.as_deref(),
                    &mut artifacts,
                )
                .await?;
                push_observation(&mut transcript, &mut artifacts, &observation);
                remember_observation(&request, AgentId::CreateProject, &observation).await;
            }
            ReactActionKind::ListWorkspace => {
                log_tool_call(
                    AgentId::Yazg,
                    "list_workspace",
                    "reading workspace inventory from DB",
                    request.log_ctx(),
                );
                let observation = execute_list_workspace(&request, &mut artifacts).await?;
                push_observation(&mut transcript, &mut artifacts, &observation);
                remember_observation(&request, AgentId::ListWorkspace, &observation).await;
                // Finish immediately with the DB inventory. Asking the model to echo the
                // full inventory inside JSON "reply" routinely breaks brace extraction
                // (nested JSON-in-string) and leaves the chat stuck on "Yazg is working…".
                let reply = artifacts
                    .workspace_inventory
                    .as_ref()
                    .map(|inventory| inventory.to_user_reply())
                    .filter(|text| !text.trim().is_empty())
                    .unwrap_or_else(|| observation.clone());
                artifacts.final_reply = reply.clone();
                log_tool_call(
                    AgentId::Yazg,
                    "finish",
                    truncate(&reply, 400),
                    request.log_ctx(),
                );
                remember_stm(
                    request.memory,
                    &request.memory_ctx,
                    StmWrite {
                        agent_id: AgentId::Yazg,
                        role: StmRole::Assistant,
                        memory_key: Some("reply".into()),
                        content: reply,
                        content_json: None,
                        importance: 0.6,
                    },
                )
                .await;
                persist_react_ltm(&request, &artifacts).await;
                artifacts.events.push(AgentEvent::completed(
                    AgentId::Yazg,
                    "ReAct finished",
                ));
                return Ok(artifacts);
            }
        }

        transcript.push_str(
            "\nContinue ReAct: choose the next action JSON \
             (analyze_endpoint, attack_plan, generate_prompt, recommend, summary, judge, \
             create_project, list_workspace, or finish).\n\
             Reply with one JSON object only.\n",
        );
    }

    artifacts.events.push(AgentEvent::failed(
        AgentId::Yazg,
        "ReAct reached max steps without finish",
    ));
    if let Some(inventory) = artifacts.workspace_inventory.as_ref() {
        if artifacts.final_reply.trim().is_empty()
            || !inventory.reply_covers_inventory(&artifacts.final_reply)
        {
            artifacts.final_reply = inventory.to_user_reply();
        }
    } else if artifacts.final_reply.trim().is_empty() {
        artifacts.final_reply = summarize_partial(&artifacts);
    }
    persist_react_ltm(&request, &artifacts).await;
    Ok(artifacts)
}

async fn remember_observation(
    request: &ReactRequest<'_>,
    agent_id: AgentId,
    observation: &str,
) {
    remember_stm(
        request.memory,
        &request.memory_ctx,
        StmWrite {
            agent_id,
            role: StmRole::Observation,
            memory_key: Some("observation".into()),
            content: observation.to_string(),
            content_json: None,
            importance: 0.55,
        },
    )
    .await;
}

async fn persist_react_ltm(request: &ReactRequest<'_>, artifacts: &ReactArtifacts) {
    let (scope_type, scope_id) = request.memory_ctx.primary_scope();

    if let Some(analyze) = artifacts.analyze.as_ref() {
        remember_ltm(
            request.memory,
            LtmWrite {
                agent_id: AgentId::AnalyzeEndpoint,
                scope_type,
                scope_id: scope_id.clone(),
                memory_key: "target.verification".into(),
                content: format!(
                    "verified status={} provider={} model={}",
                    analyze.verification.status_code,
                    analyze.verification.provider,
                    analyze.verification.model.as_deref().unwrap_or("unknown")
                ),
                content_json: Some(serde_json::json!({
                    "status_code": analyze.verification.status_code,
                    "provider": analyze.verification.provider,
                    "model": analyze.verification.model,
                })),
                importance: 0.85,
            },
        )
        .await;
    }

    if let Some(plan) = artifacts.plan.as_ref() {
        remember_ltm(
            request.memory,
            LtmWrite {
                agent_id: AgentId::AttackPlan,
                scope_type,
                scope_id: scope_id.clone(),
                memory_key: "target.attack_plan".into(),
                content: format!(
                    "profile={} categories={} source={}",
                    plan.plan.profile_id,
                    plan.plan.categories.len(),
                    plan.plan.planner_source
                ),
                content_json: Some(serde_json::json!({
                    "profile_id": plan.plan.profile_id,
                    "recommended_profile_id": plan.plan.recommended_profile_id,
                    "categories": plan.plan.categories.iter().map(|c| c.as_str()).collect::<Vec<_>>(),
                    "planner_source": plan.plan.planner_source,
                })),
                importance: 0.9,
            },
        )
        .await;
    }

    if let Some(recommend) = artifacts.recommend.as_ref() {
        let scope = if let Some(scan_id) = request.memory_ctx.scan_id.as_ref() {
            (MemoryScopeType::Scan, scan_id.clone())
        } else {
            (scope_type, scope_id.clone())
        };
        remember_ltm(
            request.memory,
            LtmWrite {
                agent_id: AgentId::Recommend,
                scope_type: scope.0,
                scope_id: scope.1,
                memory_key: "scan.recommendations".into(),
                content: truncate(&recommend.bundle.overview, 400),
                content_json: Some(serde_json::json!({
                    "overview": recommend.bundle.overview,
                    "count": recommend.bundle.recommendations.len(),
                })),
                importance: 0.8,
            },
        )
        .await;
    }

    if let Some(summary) = artifacts.summary.as_ref() {
        let (st, sid) = match summary.kind.as_str() {
            "project" => {
                if let Some(project_id) = request.memory_ctx.project_id.as_ref() {
                    (MemoryScopeType::Project, project_id.clone())
                } else {
                    (scope_type, scope_id.clone())
                }
            }
            "scan" => {
                if let Some(scan_id) = request.memory_ctx.scan_id.as_ref() {
                    (MemoryScopeType::Scan, scan_id.clone())
                } else {
                    (scope_type, scope_id.clone())
                }
            }
            _ => (scope_type, scope_id.clone()),
        };
        remember_ltm(
            request.memory,
            LtmWrite {
                agent_id: AgentId::Summary,
                scope_type: st,
                scope_id: sid,
                memory_key: format!("summary.{}", summary.kind),
                content: truncate(&summary.bundle.overview, 400),
                content_json: Some(serde_json::json!({
                    "kind": summary.kind,
                    "overview": summary.bundle.overview,
                    "highlights": summary.bundle.highlights.len(),
                })),
                importance: 0.75,
            },
        )
        .await;
    }

    if let Some(gen) = artifacts.generate_prompt.as_ref() {
        remember_ltm(
            request.memory,
            LtmWrite {
                agent_id: AgentId::GeneratePrompt,
                scope_type: MemoryScopeType::Global,
                scope_id: String::new(),
                memory_key: format!("factory.{}", gen.technique_id),
                content: truncate(&gen.content, 400),
                content_json: Some(serde_json::json!({
                    "technique_id": gen.technique_id,
                    "chars": gen.content.chars().count(),
                })),
                importance: 0.65,
            },
        )
        .await;
    }

    if let Some(judge) = artifacts.judge.as_ref() {
        remember_ltm(
            request.memory,
            LtmWrite {
                agent_id: AgentId::JudgeCoordinator,
                scope_type,
                scope_id,
                memory_key: "judge.last_verdict".into(),
                content: format!(
                    "vulnerable={} confidence={:.2}",
                    judge.verdict.vulnerable, judge.verdict.confidence
                ),
                content_json: Some(serde_json::json!({
                    "vulnerable": judge.verdict.vulnerable,
                    "confidence": judge.verdict.confidence,
                })),
                importance: 0.7,
            },
        )
        .await;
    }
}

fn push_observation(
    transcript: &mut String,
    artifacts: &mut ReactArtifacts,
    observation: &str,
) {
    transcript.push_str(&format!("Observation:\n{observation}\n"));
    artifacts.events.push(AgentEvent::react(
        AgentId::Yazg,
        format!("Observation: {}", truncate(observation, 240)),
    ));
}

fn format_context(
    profile: Option<&TargetProfile>,
    has_probe: bool,
    technique: Option<&TechniquePromptContext>,
    has_attack_results: bool,
    summary_request: Option<&SummaryRequest>,
    judge_ready: bool,
) -> String {
    let mut out = match profile {
        Some(p) => format!(
            "Runtime context:\n- bound_target: {}\n- provider: {}\n- verified: {}\n- capability_probe_ready: {}\n",
            p.full_url(),
            p.provider.as_str(),
            p.is_verified(),
            has_probe
        ),
        None => "Runtime context:\n- bound_target: (none — resolve from prompt + STM/LTM, or ask the user)\n- verified: false\n- capability_probe_ready: false\n"
            .into(),
    };
    if has_probe {
        out.push_str(
            "- note: capability_probe_ready means Scan wizard Verification — call analyze_endpoint; \
             this is NOT Attack Factory / generate_prompt\n",
        );
    }
    if let Some(t) = technique {
        out.push_str(&format!(
            "- technique_id: {}\n- technique_name: {}\n- category: {}\n- factory_prompt_ready: true\n\
             - note: Attack Factory generate_prompt does not need a scan target\n",
            t.id, t.name, t.category_id
        ));
    } else {
        out.push_str("- factory_prompt_ready: false\n");
    }
    out.push_str(&format!(
        "- attack_results_ready: {}\n",
        has_attack_results
    ));
    if has_attack_results {
        out.push_str(
            "- note: recommend uses completed scan results; no live target probe needed\n",
        );
    }
    match summary_request {
        Some(SummaryRequest::Project { project_name, .. }) => {
            out.push_str(&format!(
                "- summary_ready: true\n- summary_kind: project\n- project_name: {project_name}\n\
                 - note: summary does not need a live scan target\n"
            ));
        }
        Some(SummaryRequest::Scan { .. }) => {
            out.push_str(
                "- summary_ready: true\n- summary_kind: scan\n\
                 - note: summary uses completed scan results; no live target probe needed\n",
            );
        }
        None => out.push_str("- summary_ready: false\n"),
    }
    out.push_str(&format!("- judge_ready: {judge_ready}\n"));
    if judge_ready {
        out.push_str(
            "- note: judge runs JudgeCoordinatorAgent → JudgeWorker/ClassifierWorker/AttackerWorker; \
             no live target probe needed when probe response context is present\n",
        );
    }
    out.push_str(
        "- create_project_ready: true\n\
         - note: create_project only needs a project name (+ optional description). \
         Do NOT require a scan target or analyze_endpoint for project creation.\n\
         - list_workspace_ready: true\n\
         - note: list_workspace reads projects/targets/scans/findings from the local DB. \
         Call it once when the user asks what exists in the workspace. The tool returns the \
         inventory as the final reply — do not invent rows and do not call it twice.\n",
    );
    out
}

fn parse_action_kind(raw: &str) -> AgentResult<ReactActionKind> {
    match raw.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "analyze_endpoint" | "analyze" | "verify" | "verification" => {
            Ok(ReactActionKind::AnalyzeEndpoint)
        }
        "plan" | "planner" | "attack_plan" => Ok(ReactActionKind::AttackPlan),
        "generate_prompt"
        | "generate"
        | "prompt"
        | "factory_prompt"
        | "attack_factory" => Ok(ReactActionKind::GeneratePrompt),
        "recommend" | "recommendation" | "recommendations" | "remediation" => {
            Ok(ReactActionKind::Recommend)
        }
        "summary" | "summarize" | "project_summary" | "scan_summary" => {
            Ok(ReactActionKind::Summary)
        }
        "judge" | "judging" | "judge_coordinator" | "consensus_judge" => {
            Ok(ReactActionKind::Judge)
        }
        "create_project" | "createproject" | "new_project" | "add_project" => {
            Ok(ReactActionKind::CreateProject)
        }
        "list_workspace"
        | "list-workspace"
        | "workspace"
        | "list_projects"
        | "inventory"
        | "db_inventory" => Ok(ReactActionKind::ListWorkspace),
        "finish" | "done" | "respond" | "final" => Ok(ReactActionKind::Finish),
        other => Err(AgentError::Supervisor(format!(
            "unknown ReAct action '{other}'"
        ))),
    }
}

fn parse_react_step(raw: &str) -> Result<ReactStepJson, String> {
    let cleaned = strip_markdown_fences(raw.trim());
    let json_slice = extract_json_object(cleaned.trim()).ok_or_else(|| {
        "ReAct response did not contain a JSON object".to_string()
    })?;
    serde_json::from_str(json_slice).map_err(|err| format!("invalid ReAct JSON: {err}"))
}

/// Strip ``` / ```json wrappers so models that wrap JSON still parse.
fn strip_markdown_fences(raw: &str) -> String {
    let trimmed = raw.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }
    let mut lines = trimmed.lines();
    let _ = lines.next(); // opening fence
    let mut body: Vec<&str> = lines.collect();
    if body
        .last()
        .is_some_and(|line| line.trim().starts_with("```"))
    {
        body.pop();
    }
    body.join("\n")
}

fn extract_json_object(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    for (idx, ch) in raw[start..].char_indices() {
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&raw[start..start + idx + ch.len_utf8()]);
                }
            }
            _ => {}
        }
    }
    None
}

async fn execute_analyze(
    request: &ReactRequest<'_>,
    analyze_llm: &dyn PlannerLlm,
    artifacts: &mut ReactArtifacts,
) -> AgentResult<String> {
    let Some(profile) = request.profile else {
        return Ok("AnalyzeEndpointAgent FAILED — no target selected".into());
    };

    artifacts.events.push(AgentEvent::info(
        AgentId::Yazg,
        "Acting: AnalyzeEndpointAgent",
    ));

    let outcome = if let Some(http) = request.capability_probe {
        AnalyzeEndpointAgent::classify_probe(profile, http, analyze_llm).await
    } else {
        AnalyzeEndpointAgent::run(profile, request.auth_headers.clone(), analyze_llm).await
    };

    match outcome {
        Ok(out) => {
            artifacts.events.extend(out.events.clone());
            let msg = format!(
                "AnalyzeEndpointAgent OK — verified=true status={} latency_ms={} provider={} model={}",
                out.verification.status_code,
                out.verification.response_time_ms,
                out.verification.provider,
                out.verification.model.as_deref().unwrap_or("unknown")
            );
            artifacts.analyze = Some(out);
            Ok(msg)
        }
        Err(err) => {
            artifacts.events.push(AgentEvent::failed(
                AgentId::AnalyzeEndpoint,
                err.to_string(),
            ));
            Ok(format!("AnalyzeEndpointAgent FAILED — {err}"))
        }
    }
}

async fn execute_attack_plan(
    request: &ReactRequest<'_>,
    plan_llm: &dyn PlannerLlm,
    artifacts: &mut ReactArtifacts,
) -> AgentResult<String> {
    let Some(profile) = request.profile else {
        return Ok("AttackPlanAgent FAILED — no target selected".into());
    };

    artifacts
        .events
        .push(AgentEvent::info(AgentId::Yazg, "Acting: AttackPlanAgent"));

    match AttackPlanAgent::run(profile, plan_llm).await {
        Ok(out) => {
            artifacts.events.extend(out.events.clone());
            let msg = format!(
                "AttackPlanAgent OK — categories={} modes={} source={} summary={}",
                out.plan.categories.len(),
                out.plan.profile_modes.len(),
                out.plan.planner_source,
                truncate(&out.plan.summary, 160)
            );
            artifacts.plan = Some(out);
            Ok(msg)
        }
        Err(err) => {
            artifacts
                .events
                .push(AgentEvent::failed(AgentId::AttackPlan, err.to_string()));
            Ok(format!("AttackPlanAgent FAILED — {err}"))
        }
    }
}

async fn execute_generate_prompt(
    request: &ReactRequest<'_>,
    prompt_llm: &dyn PlannerLlm,
    artifacts: &mut ReactArtifacts,
) -> AgentResult<String> {
    let Some(technique) = request.technique else {
        return Ok("GeneratePromptAgent FAILED — no technique selected".into());
    };

    artifacts.events.push(AgentEvent::info(
        AgentId::Yazg,
        "Acting: GeneratePromptAgent",
    ));

    match GeneratePromptAgent::run(technique, prompt_llm).await {
        Ok(out) => {
            artifacts.events.extend(out.events.clone());
            let msg = format!(
                "GeneratePromptAgent OK — technique={} chars={} preview={}",
                out.technique_id,
                out.content.chars().count(),
                truncate(&out.content, 120)
            );
            artifacts.generate_prompt = Some(out);
            Ok(msg)
        }
        Err(err) => {
            artifacts.events.push(AgentEvent::failed(
                AgentId::GeneratePrompt,
                err.to_string(),
            ));
            Ok(format!("GeneratePromptAgent FAILED — {err}"))
        }
    }
}

async fn execute_recommend(
    request: &ReactRequest<'_>,
    recommend_llm: &dyn PlannerLlm,
    artifacts: &mut ReactArtifacts,
) -> AgentResult<String> {
    let Some(results) = request.attack_results else {
        return Ok("RecommendAgent FAILED — no attack results provided".into());
    };

    artifacts.events.push(AgentEvent::info(
        AgentId::Yazg,
        "Acting: RecommendAgent",
    ));

    match RecommendAgent::run(results, recommend_llm).await {
        Ok(out) => {
            artifacts.events.extend(out.events.clone());
            let msg = format!(
                "RecommendAgent OK — items={} overview={}",
                out.bundle.recommendations.len(),
                truncate(&out.bundle.overview, 160)
            );
            artifacts.recommend = Some(out);
            Ok(msg)
        }
        Err(err) => {
            artifacts.events.push(AgentEvent::failed(
                AgentId::Recommend,
                err.to_string(),
            ));
            Ok(format!("RecommendAgent FAILED — {err}"))
        }
    }
}

async fn execute_summary(
    request: &ReactRequest<'_>,
    summary_llm: &dyn PlannerLlm,
    artifacts: &mut ReactArtifacts,
) -> AgentResult<String> {
    let Some(summary_request) = request.summary_request else {
        return Ok("SummaryAgent FAILED — no summary request provided".into());
    };

    artifacts.events.push(AgentEvent::info(
        AgentId::Yazg,
        "Acting: SummaryAgent",
    ));

    match SummaryAgent::run(summary_request, summary_llm).await {
        Ok(out) => {
            artifacts.events.extend(out.events.clone());
            let msg = format!(
                "SummaryAgent OK — kind={} overview={} highlights={}",
                out.kind,
                truncate(&out.bundle.overview, 120),
                out.bundle.highlights.len()
            );
            artifacts.summary = Some(out);
            Ok(msg)
        }
        Err(err) => {
            artifacts
                .events
                .push(AgentEvent::failed(AgentId::Summary, err.to_string()));
            Ok(format!("SummaryAgent FAILED — {err}"))
        }
    }
}

async fn execute_judge(
    request: &ReactRequest<'_>,
    artifacts: &mut ReactArtifacts,
) -> AgentResult<String> {
    let (Some(judge_request), Some(engine)) = (request.judge_request, request.judge_engine) else {
        return Ok(
            "JudgeCoordinatorAgent FAILED — judge request/engine not provided".into(),
        );
    };

    artifacts.events.push(AgentEvent::info(
        AgentId::Yazg,
        "Acting: JudgeCoordinatorAgent",
    ));

    match JudgeCoordinatorAgent::run(judge_request, engine).await {
        Ok(out) => {
            artifacts.events.extend(out.events.clone());
            let msg = format!(
                "JudgeCoordinatorAgent OK — verdict={} confidence={:.2} votes={}",
                out.verdict.verdict,
                out.verdict.confidence,
                out.worker_results.len()
            );
            artifacts.judge = Some(out);
            Ok(msg)
        }
        Err(err) => {
            artifacts.events.push(AgentEvent::failed(
                AgentId::JudgeCoordinator,
                err.to_string(),
            ));
            Ok(format!("JudgeCoordinatorAgent FAILED — {err}"))
        }
    }
}

async fn execute_create_project(
    request: &ReactRequest<'_>,
    name: &str,
    description: Option<&str>,
    artifacts: &mut ReactArtifacts,
) -> AgentResult<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Ok(
            "CreateProjectTool FAILED — missing project name. \
             Call create_project again with JSON field \"name\"."
                .into(),
        );
    }

    let Some(tools) = request.project_tools else {
        return Ok(
            "CreateProjectTool FAILED — project tools unavailable in this context".into(),
        );
    };

    artifacts.events.push(AgentEvent::info(
        AgentId::Yazg,
        "Acting: CreateProjectTool",
    ));

    let description = description
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match tools.create_project(trimmed, description).await {
        Ok(project) => {
            let msg = format!(
                "CreateProjectTool OK — id={} name={} description={}",
                project.id,
                project.name,
                project.description.as_deref().unwrap_or("(none)")
            );
            artifacts.events.push(AgentEvent::completed(
                AgentId::CreateProject,
                format!("Created project {}", project.name),
            ));
            artifacts.created_project = Some(project);
            Ok(msg)
        }
        Err(err) => {
            artifacts
                .events
                .push(AgentEvent::failed(AgentId::CreateProject, err.clone()));
            Ok(format!("CreateProjectTool FAILED — {err}"))
        }
    }
}

async fn execute_list_workspace(
    request: &ReactRequest<'_>,
    artifacts: &mut ReactArtifacts,
) -> AgentResult<String> {
    if let Some(existing) = artifacts.workspace_inventory.as_ref() {
        return Ok(format!(
            "{}\n(note: reused previous ListWorkspaceTool Observation — do not call again; finish with this inventory)",
            existing.to_observation()
        ));
    }

    let Some(tools) = request.workspace_tools else {
        return Ok(
            "ListWorkspaceTool FAILED — workspace tools unavailable in this context".into(),
        );
    };

    artifacts.events.push(AgentEvent::info(
        AgentId::Yazg,
        "Acting: ListWorkspaceTool",
    ));

    match tools.list_workspace().await {
        Ok(inventory) => {
            let msg = inventory.to_observation();
            artifacts.events.push(AgentEvent::completed(
                AgentId::ListWorkspace,
                format!(
                    "Listed workspace: {} projects, {} targets, {} scans, {} findings",
                    inventory.totals.projects,
                    inventory.totals.targets,
                    inventory.totals.scans,
                    inventory.totals.findings
                ),
            ));
            artifacts.workspace_inventory = Some(inventory);
            Ok(msg)
        }
        Err(err) => {
            artifacts
                .events
                .push(AgentEvent::failed(AgentId::ListWorkspace, err.clone()));
            Ok(format!("ListWorkspaceTool FAILED — {err}"))
        }
    }
}

fn summarize_partial(artifacts: &ReactArtifacts) -> String {
    if let Some(inventory) = &artifacts.workspace_inventory {
        return inventory.to_user_reply();
    }
    if let Some(project) = &artifacts.created_project {
        return format!(
            "Reached step limit after creating project '{}' (id={}).",
            project.name, project.id
        );
    }
    if let Some(judge) = &artifacts.judge {
        return format!(
            "Reached step limit after judging. Verdict: {} · confidence={:.2}",
            judge.verdict.verdict, judge.verdict.confidence
        );
    }
    if let Some(summary) = &artifacts.summary {
        return format!(
            "Reached step limit after summary. Kind: {} · {}",
            summary.kind,
            truncate(&summary.bundle.overview, 120)
        );
    }
    if let Some(rec) = &artifacts.recommend {
        return format!(
            "Reached step limit after recommendations. Items: {}",
            rec.bundle.recommendations.len()
        );
    }
    if let Some(gen) = &artifacts.generate_prompt {
        return format!(
            "Reached step limit after prompt generation. Technique: {} · {} chars",
            gen.technique_id,
            gen.content.chars().count()
        );
    }
    if let Some(plan) = &artifacts.plan {
        return format!(
            "Reached step limit after planning. Categories: {} · source: {}",
            plan.plan.categories.len(),
            plan.plan.planner_source
        );
    }
    if let Some(analyze) = &artifacts.analyze {
        return format!(
            "Reached step limit after endpoint analysis. Verified AI endpoint (HTTP {}).",
            analyze.verification.status_code
        );
    }
    "I could not finish the ReAct loop in time. Try again or narrow the request.".into()
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
    fn parses_react_json() {
        let raw = r#"Here is my step:
{"thought":"Need to classify the API","action":"analyze_endpoint"}
"#;
        let step = parse_react_step(raw).expect("parse");
        assert_eq!(step.action, "analyze_endpoint");
        assert!(step.thought.unwrap().contains("classify"));
    }

    #[test]
    fn parses_react_json_inside_markdown_fence() {
        let raw = "```json\n{\"thought\":\"Re-plan\",\"action\":\"attack_plan\"}\n```";
        let step = parse_react_step(raw).expect("parse fenced");
        assert_eq!(step.action, "attack_plan");
    }

    #[test]
    fn parses_react_json_with_nested_json_string_reply() {
        let raw = r#"{"thought":"done","action":"finish","reply":"{\n  \"projects\": [\n    {\"id\": \"1\", \"name\": \"AI\"}\n  ]\n}"}"#;
        let step = parse_react_step(raw).expect("parse nested reply string");
        assert_eq!(step.action, "finish");
        assert!(step.reply.unwrap().contains("projects"));
    }

    #[test]
    fn maps_action_aliases() {
        assert_eq!(
            parse_action_kind("verify").unwrap(),
            ReactActionKind::AnalyzeEndpoint
        );
        assert_eq!(parse_action_kind("finish").unwrap(), ReactActionKind::Finish);
        assert_eq!(
            parse_action_kind("plan").unwrap(),
            ReactActionKind::AttackPlan
        );
        assert_eq!(
            parse_action_kind("recommend").unwrap(),
            ReactActionKind::Recommend
        );
        assert_eq!(
            parse_action_kind("project_summary").unwrap(),
            ReactActionKind::Summary
        );
        assert_eq!(
            parse_action_kind("judge").unwrap(),
            ReactActionKind::Judge
        );
        assert_eq!(
            parse_action_kind("create_project").unwrap(),
            ReactActionKind::CreateProject
        );
        assert_eq!(
            parse_action_kind("new_project").unwrap(),
            ReactActionKind::CreateProject
        );
        assert_eq!(
            parse_action_kind("list_workspace").unwrap(),
            ReactActionKind::ListWorkspace
        );
        assert_eq!(
            parse_action_kind("inventory").unwrap(),
            ReactActionKind::ListWorkspace
        );
        assert_eq!(
            parse_action_kind("generate_prompt").unwrap(),
            ReactActionKind::GeneratePrompt
        );
    }
}
