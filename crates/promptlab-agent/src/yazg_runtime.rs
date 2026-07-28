//! Yazg supervisor runtime.
//!
//! Yazg is the manager agent; specialists are **domain tools** (not nested LLM workers)
//! that call AnalyzeEndpointAgent / SummaryAgent / … directly.
//!
//! Pattern follows Rig examples (`agent_with_tools`, `multi_turn_agent`,
//! `gemini_default_api_recovery`): AgentBuilder + tools + preamble + InvalidToolCall hook.

use std::sync::Arc;

use rig::agent::{AgentBuilder, AgentHook, Flow, HookContext, StepEvent};
use rig::completion::Prompt;
use tracing::{info, warn};

use crate::artifacts::{persist_artifacts_ltm, YazgActionKind, YazgArtifacts};
use crate::create_project::CreateProjectTools;
use crate::error::{AgentError, AgentResult};
use crate::list_workspace::WorkspaceTools;
use crate::memory::{
    load_memory_prompt_block, remember_stm, AgentMemoryStore, MemoryContext, StmRole, StmWrite,
};
use crate::types::{AgentEvent, AgentId};
use crate::yazg_model::YazgModel;
use crate::yazg_tools::{
    AnalyzeEndpointTool, AttackPlanTool, CreateProjectTool, GeneratePromptTool, JudgeTool,
    ListWorkspaceTool, RecommendTool, SharedYazgState, SummaryTool, YazgLlms, YazgRunState,
    YazgSpecialistContext,
};

const DEFAULT_MAX_TURNS: usize = 6;

/// Core system preamble (mirrors PromptRegistry::yazg_react_system; kept here so
/// Rig AgentBuilder owns the system prompt end-to-end like provider examples).
const YAZG_PREAMBLE: &str = r#"You are Yazg, the PromptLab supervisor agent and in-app AI assistant.

Identity: When asked who you are, introduce yourself as Yazg — PromptLab's AI assistant for authorized AI security testing (endpoint analysis, attack planning, prompt generation, judging, and workspace help).

Tools are provided via the API tool-calling interface. Read each tool description and call the single best tool for the user goal, or respond directly with assistant text when no tool is needed.

Rules:
- User-visible replies MUST be markdown or plain text only. Never emit JSON, tool envelopes, function-call objects, or code that looks like `{"name":"assistant_reply",...}`.
- Greetings (hi/hello), identity questions (who are you), thanks, and small talk → natural assistant text. Do not mention tools or internal routing.
- Never invent tools. Only call names from the bound tool list. There is no assistant_reply / final_answer — plain text is the reply.
- Forbidden in final replies: tool/tool-call mentions, ReAct/Observation/step logs, routing notes, or "I need to call tool..." phrasing.
- Only call a specialist tool when the user request clearly needs it.
- Prefer the smallest useful action; never invent tool results.
- create_project needs a name; list_workspace only for explicit DB inventory questions.
- After an Observation, either take another tool call or respond with a clear user reply."#;

/// Rig-style recovery when the model invents a tool name (see `gemini_default_api_recovery`).
#[derive(Clone, Default)]
struct YazgInvalidToolHook;

impl AgentHook<YazgModel> for YazgInvalidToolHook {
    async fn on_event(&self, _ctx: &HookContext, event: StepEvent<'_, YazgModel>) -> Flow {
        match event {
            StepEvent::InvalidToolCall(inv) => {
                warn!(tool = %inv.tool_name, "invalid tool call; asking model to reply in text");
                Flow::retry(format!(
                    "`{}` is not a valid tool. Available tools: [{}]. \
                     Reply to the user in plain text, or call one of the available tools.",
                    inv.tool_name,
                    inv.available_tools.join(", ")
                ))
            }
            _ => Flow::Continue,
        }
    }
}

/// Owned context for a Yazg run (chat / tool-calling / wizard).
pub struct YazgRequest {
    pub goal: String,
    pub memory: Option<Arc<dyn AgentMemoryStore>>,
    pub memory_ctx: MemoryContext,
    pub workspace_tools: Option<Arc<dyn WorkspaceTools>>,
    pub project_tools: Option<Arc<dyn CreateProjectTools>>,
    pub specialist: YazgSpecialistContext,
    pub llms: YazgLlms,
    pub max_turns: usize,
}

impl YazgRequest {
    pub fn new(goal: impl Into<String>, llms: YazgLlms) -> Self {
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

/// Run Yazg tool-calling loop and return artifacts + raw PromptResponse JSON.
pub async fn run_yazg(request: YazgRequest) -> AgentResult<(YazgArtifacts, serde_json::Value)> {
    let mut artifacts = YazgArtifacts::default();
    artifacts.events.push(AgentEvent::started(
        AgentId::Yazg,
        "Yazg agent loop started",
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

    let state: SharedYazgState = Arc::new(tokio::sync::Mutex::new(YazgRunState::default()));
    let specialist = Arc::new(request.specialist);
    let context_block = specialist.format_context_block();
    let model = YazgModel::new(request.llms.supervisor.clone());

    let preamble = format!("{YAZG_PREAMBLE}\n\n{context_block}\n{memory_block}");

    // Domain tools call specialist ::run() directly (no nested worker LLM).
    let mut builder = AgentBuilder::new(model)
        .name("Yazg")
        .description("PromptLab AI Assistant")
        .preamble(&preamble)
        .temperature(0.2)
        .max_tokens(1024)
        .default_max_turns(request.max_turns)
        .add_hook(YazgInvalidToolHook)
        .tool(AnalyzeEndpointTool {
            ctx: specialist.clone(),
            llm: request.llms.analyze.clone(),
            state: state.clone(),
        });

    if let Some(tools) = request.workspace_tools.clone() {
        builder = builder.tool(ListWorkspaceTool {
            tools,
            state: state.clone(),
        });
    }
    if let Some(tools) = request.project_tools.clone() {
        builder = builder.tool(CreateProjectTool {
            tools,
            state: state.clone(),
        });
    }

    let agent = builder
        .tool(AttackPlanTool {
            ctx: specialist.clone(),
            llm: request.llms.plan.clone(),
            state: state.clone(),
        })
        .tool(GeneratePromptTool {
            ctx: specialist.clone(),
            llm: request.llms.prompt.clone(),
            state: state.clone(),
        })
        .tool(RecommendTool {
            ctx: specialist.clone(),
            llm: request.llms.recommend.clone(),
            state: state.clone(),
        })
        .tool(SummaryTool {
            ctx: specialist.clone(),
            llm: request.llms.summary.clone(),
            state: state.clone(),
        })
        .tool(JudgeTool {
            ctx: specialist,
            orchestrator: request.llms.supervisor.clone(),
            state: state.clone(),
        })
        .build();

    info!(goal = %truncate(&request.goal, 120), "Yazg agent handle");

    let prompt_result = agent
        .prompt(request.goal.trim())
        .max_turns(request.max_turns)
        .extended_details()
        .await;

    let (reply_text, raw_output, usage_note) = match prompt_result {
        Ok(prompt_response) => {
            let raw_output = serde_json::to_value(&prompt_response).unwrap_or_else(|_| {
                serde_json::json!({ "output": prompt_response.output.clone() })
            });
            let usage_note = format!(
                "Yazg usage: in={} out={} requests={}",
                prompt_response.usage.input_tokens,
                prompt_response.usage.output_tokens,
                prompt_response.completion_calls.len()
            );
            (
                prompt_response.output.trim().to_string(),
                raw_output,
                usage_note,
            )
        }
        Err(err) => {
            let msg = err.to_string();
            if msg.contains("MaxTurnsError") {
                warn!(error = %err, "Yazg hit max turns; salvaging specialist artifacts");
                (
                    String::new(),
                    serde_json::json!({ "error": "max_turns", "detail": msg }),
                    format!("Yazg max-turns salvage: {msg}"),
                )
            } else {
                warn!(error = %err, "Yazg agent loop failed");
                return Err(AgentError::Supervisor(format!(
                    "Yazg agent loop failed: {err}"
                )));
            }
        }
    };

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

    artifacts.final_reply = if reply_text.is_empty() {
        if let Some(inventory) = artifacts.workspace_inventory.as_ref() {
            inventory.to_user_reply_for_goal(&request.goal)
        } else if artifacts.summary.is_some()
            || artifacts.plan.is_some()
            || artifacts.analyze.is_some()
            || artifacts.recommend.is_some()
            || artifacts.generate_prompt.is_some()
            || artifacts.judge.is_some()
            || artifacts.created_project.is_some()
        {
            "Specialist finished; synthesizing UI reply.".into()
        } else {
            "Yazg finished without a reply.".into()
        }
    } else {
        crate::yazg_model::normalize_user_facing_reply(&reply_text)
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
        "Yazg agent loop finished",
    ));
    artifacts.events.push(AgentEvent::info(AgentId::Yazg, usage_note));

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
