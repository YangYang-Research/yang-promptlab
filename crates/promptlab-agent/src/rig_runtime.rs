//! Rig-based Yazg supervisor runtime — manager–worker (agents as tools).
//!
//! Per Rig Book *Multi-agent systems*:
//! specialists are named/described Rig Agents attached with `.tool(worker)`.

use std::sync::Arc;

use rig::agent::AgentBuilder;
use rig::completion::Prompt;
use tracing::{info, warn};

use crate::create_project::CreateProjectTools;
use crate::error::{AgentError, AgentResult};
use crate::list_workspace::WorkspaceTools;
use crate::memory::{
    load_memory_prompt_block, remember_stm, AgentMemoryStore, MemoryContext, StmRole, StmWrite,
};
use crate::artifacts::{persist_artifacts_ltm, YazgActionKind, YazgArtifacts};
use crate::rig_model::YazgRigModel;
use crate::rig_tools::{SharedYazgRigState, YazgRigLlms, YazgRigRunState, YazgSpecialistContext};
use crate::rig_workers::{
    build_analyze_endpoint_worker, build_attack_plan_worker, build_create_project_worker,
    build_generate_prompt_worker, build_judge_worker, build_list_workspace_worker,
    build_recommend_worker, build_summary_worker,
};
use crate::types::{AgentEvent, AgentId};

const DEFAULT_MAX_TURNS: usize = 6;

/// Owned context for a Rig Yazg run (chat / tool-calling / wizard).
pub struct YazgRigRequest {
    pub goal: String,
    pub memory: Option<Arc<dyn AgentMemoryStore>>,
    pub memory_ctx: MemoryContext,
    pub workspace_tools: Option<Arc<dyn WorkspaceTools>>,
    pub project_tools: Option<Arc<dyn CreateProjectTools>>,
    pub specialist: YazgSpecialistContext,
    pub llms: YazgRigLlms,
    pub max_turns: usize,
}

impl YazgRigRequest {
    pub fn new(goal: impl Into<String>, llms: YazgRigLlms) -> Self {
        Self {
            goal: goal.into(),
            memory: None,
            memory_ctx: MemoryContext::default(),
            workspace_tools: None,
            project_tools: None,
            specialist: YazgSpecialistContext::default(),
            llms,
            max_turns: DEFAULT_MAX_TURNS,
        }
    }

    pub fn with_memory(
        mut self,
        store: Option<Arc<dyn AgentMemoryStore>>,
        ctx: MemoryContext,
    ) -> Self {
        self.memory = store;
        self.memory_ctx = ctx;
        self
    }

    pub fn with_workspace_tools(mut self, tools: Option<Arc<dyn WorkspaceTools>>) -> Self {
        self.workspace_tools = tools;
        self
    }

    pub fn with_project_tools(mut self, tools: Option<Arc<dyn CreateProjectTools>>) -> Self {
        self.project_tools = tools;
        self
    }

    pub fn with_specialist(mut self, specialist: YazgSpecialistContext) -> Self {
        self.specialist = specialist;
        self
    }

    pub fn with_max_turns(mut self, max_turns: usize) -> Self {
        self.max_turns = max_turns.max(1);
        self
    }
}

/// Run Yazg via Rig manager–worker agents and return artifacts + raw PromptResponse JSON.
pub async fn run_yazg_rig(request: YazgRigRequest) -> AgentResult<(YazgArtifacts, serde_json::Value)> {
    let mut artifacts = YazgArtifacts::default();
    artifacts.events.push(AgentEvent::started(
        AgentId::Yazg,
        "Rig manager–worker loop started",
    ));

    remember_stm(
        request.memory.as_deref(),
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

    let memory_block = load_memory_prompt_block(
        request.memory.as_deref(),
        &request.memory_ctx,
        AgentId::Yazg,
        None,
    )
    .await;

    let state: SharedYazgRigState = Arc::new(tokio::sync::Mutex::new(YazgRigRunState::default()));
    let specialist = Arc::new(request.specialist);
    let context_block = specialist.format_context_block();
    let model = YazgRigModel::new(request.llms.supervisor.clone());
    let supervisor_llm = request.llms.supervisor.clone();

    let preamble = format!(
        "You are Yazg, the manager agent for PromptLab's in-app AI Assistant.\n\
         Specialist workers are bound as tools — each tool is a sub-agent.\n\
         When delegating, call the worker with JSON `{{\"prompt\": \"<task for the worker>\"}}`.\n\
         For greetings, math, and general chat, answer directly in natural language.\n\
         Only delegate to a specialist when the user request clearly requires it.\n\
         Never mention tools, routing, ReAct, or internal decisions to the user.\n\
         Do not invent worker results.\n\n\
         {context_block}\n\
         {memory_block}"
    );

    // Manager–worker: first `.tool` transitions builder; specialists are Rig Agents.
    let mut builder = AgentBuilder::new(model)
        .name("Yazg")
        .description("PromptLab AI Assistant manager (delegates to specialist worker agents)")
        .preamble(&preamble)
        .temperature(0.2)
        .max_tokens(1024)
        .default_max_turns(request.max_turns)
        .tool(build_analyze_endpoint_worker(
            request.llms.analyze.clone(),
            specialist.clone(),
            state.clone(),
        ));

    if let Some(tools) = request.workspace_tools.clone() {
        builder = builder.tool(build_list_workspace_worker(
            supervisor_llm.clone(),
            tools,
            state.clone(),
        ));
    }
    if let Some(tools) = request.project_tools.clone() {
        builder = builder.tool(build_create_project_worker(
            supervisor_llm.clone(),
            tools,
            state.clone(),
        ));
    }

    let agent = builder
        .tool(build_attack_plan_worker(
            request.llms.plan.clone(),
            specialist.clone(),
            state.clone(),
        ))
        .tool(build_generate_prompt_worker(
            request.llms.prompt.clone(),
            specialist.clone(),
            state.clone(),
        ))
        .tool(build_recommend_worker(
            request.llms.recommend.clone(),
            specialist.clone(),
            state.clone(),
        ))
        .tool(build_summary_worker(
            request.llms.summary.clone(),
            specialist.clone(),
            state.clone(),
        ))
        .tool(build_judge_worker(
            request.llms.supervisor.clone(),
            specialist,
            state.clone(),
        ))
        .build();

    info!(goal = %truncate(&request.goal, 120), "Yazg Rig manager–worker handle");

    let prompt_response = agent
        .prompt(request.goal.trim())
        .max_turns(request.max_turns)
        .extended_details()
        .await
        .map_err(|err| {
            warn!(error = %err, "Yazg Rig prompt failed");
            AgentError::Supervisor(format!("Rig agent failed: {err}"))
        })?;

    let raw_output = serde_json::to_value(&prompt_response).unwrap_or_else(|_| {
        serde_json::json!({ "output": prompt_response.output.clone() })
    });

    let mut run_state = state.lock().await;
    artifacts.events.append(&mut run_state.artifacts.events);
    artifacts.workspace_inventory = run_state.artifacts.workspace_inventory.take();
    artifacts.created_project = run_state.artifacts.created_project.take();
    artifacts.analyze = run_state.artifacts.analyze.take();
    artifacts.plan = run_state.artifacts.plan.take();
    artifacts.generate_prompt = run_state.artifacts.generate_prompt.take();
    artifacts.recommend = run_state.artifacts.recommend.take();
    artifacts.summary = run_state.artifacts.summary.take();
    artifacts.judge = run_state.artifacts.judge.take();
    artifacts.last_action = run_state
        .last_tool
        .as_deref()
        .and_then(map_tool_to_action)
        .or(run_state.artifacts.last_action);

    let reply = prompt_response.output.trim().to_string();
    artifacts.final_reply = if reply.is_empty() {
        if let Some(inventory) = artifacts.workspace_inventory.as_ref() {
            inventory.to_user_reply_for_goal(&request.goal)
        } else {
            "Yazg finished without a reply.".into()
        }
    } else {
        reply
    };

    remember_stm(
        request.memory.as_deref(),
        &request.memory_ctx,
        StmWrite {
            agent_id: AgentId::Yazg,
            role: StmRole::Assistant,
            memory_key: Some("reply".into()),
            content: artifacts.final_reply.clone(),
            content_json: None,
            importance: 0.6,
        },
    )
    .await;

    persist_artifacts_ltm(
        request.memory.as_deref(),
        &request.memory_ctx,
        &artifacts,
    )
    .await;

    artifacts.events.push(AgentEvent::completed(
        AgentId::Yazg,
        "Rig manager–worker finished",
    ));
    artifacts.events.push(AgentEvent::info(
        AgentId::Yazg,
        format!(
            "Rig usage: in={} out={} requests={}",
            prompt_response.usage.input_tokens,
            prompt_response.usage.output_tokens,
            prompt_response.completion_calls.len()
        ),
    ));

    Ok((artifacts, raw_output))
}

fn map_tool_to_action(name: &str) -> Option<YazgActionKind> {
    match name {
        "list_workspace" => Some(YazgActionKind::ListWorkspace),
        "create_project" => Some(YazgActionKind::CreateProject),
        "analyze_endpoint" => Some(YazgActionKind::AnalyzeEndpoint),
        "attack_plan" => Some(YazgActionKind::AttackPlan),
        "generate_prompt" => Some(YazgActionKind::GeneratePrompt),
        "recommend" => Some(YazgActionKind::Recommend),
        "summary" => Some(YazgActionKind::Summary),
        "judge" => Some(YazgActionKind::Judge),
        _ => None,
    }
}

fn truncate(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    let shortened: String = t.chars().take(max.saturating_sub(1)).collect();
    format!("{shortened}…")
}
