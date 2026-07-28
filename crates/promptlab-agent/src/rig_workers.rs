//! Rig manager–worker builders (agents registered as tools).
//!
//! Matches Rig Book *Multi-agent systems*:
//! `manager.tool(worker_agent)` where each worker has `.name()` + `.description()`.

use std::sync::Arc;

use promptlab_planner::PlannerLlm;
use rig::agent::{Agent, AgentBuilder};

use crate::create_project::CreateProjectTools;
use crate::list_workspace::WorkspaceTools;
use crate::rig_model::YazgRigModel;
use crate::rig_tools::{
    AnalyzeEndpointRigTool, AttackPlanRigTool, CreateProjectRigTool, GeneratePromptRigTool,
    JudgeRigTool, ListWorkspaceRigTool, RecommendRigTool, SharedYazgRigState, SummaryRigTool,
    YazgSpecialistContext,
};

const WORKER_MAX_TURNS: usize = 4;

fn worker_preamble(role: &str, tool_name: &str) -> String {
    format!(
        "You are {role}, a specialist worker under Yazg (the manager).\n\
         When Yazg delegates a task via a prompt, call the `{tool_name}` tool exactly once \
         to perform the work, then reply with a short factual summary of the tool result.\n\
         Do not invent results. Do not call other tools."
    )
}

/// ListWorkspace specialist agent (agent-as-tool for Yazg).
pub fn build_list_workspace_worker(
    llm: Arc<dyn PlannerLlm>,
    tools: Arc<dyn WorkspaceTools>,
    state: SharedYazgRigState,
) -> Agent<YazgRigModel> {
    AgentBuilder::new(YazgRigModel::new(llm))
        .name("list_workspace")
        .description(
            "Read projects, targets, scans, and findings from the local PromptLab database. \
             ONLY when the user explicitly asks for inventory or finding/vulnerability counts. \
             Do NOT use for greetings, math, or general chat.",
        )
        .preamble(&worker_preamble("ListWorkspaceAgent", "list_workspace"))
        .temperature(0.1)
        .max_tokens(512)
        .default_max_turns(WORKER_MAX_TURNS)
        .tool(ListWorkspaceRigTool { tools, state })
        .build()
}

/// CreateProject specialist agent (agent-as-tool for Yazg).
pub fn build_create_project_worker(
    llm: Arc<dyn PlannerLlm>,
    tools: Arc<dyn CreateProjectTools>,
    state: SharedYazgRigState,
) -> Agent<YazgRigModel> {
    AgentBuilder::new(YazgRigModel::new(llm))
        .name("create_project")
        .description(
            "Create a workspace project in the local database. Requires a project name. \
             Optional description. Do NOT ask for a scan target.",
        )
        .preamble(
            "You are CreateProjectAgent, a specialist worker under Yazg (the manager).\n\
             When Yazg delegates a task, extract the project name (and optional description) \
             from the prompt, call the `create_project` tool once with those fields, then \
             reply with a short factual summary. Do not invent results.",
        )
        .temperature(0.1)
        .max_tokens(512)
        .default_max_turns(WORKER_MAX_TURNS)
        .tool(CreateProjectRigTool { tools, state })
        .build()
}

/// AnalyzeEndpoint specialist agent (agent-as-tool for Yazg).
pub fn build_analyze_endpoint_worker(
    llm: Arc<dyn PlannerLlm>,
    ctx: Arc<YazgSpecialistContext>,
    state: SharedYazgRigState,
) -> Agent<YazgRigModel> {
    AgentBuilder::new(YazgRigModel::new(llm.clone()))
        .name("analyze_endpoint")
        .description(
            "Probe/classify whether a bound live scan target is a generative AI API \
             (AnalyzeEndpointAgent). Requires a bound target or capability_probe_ready=true \
             (Scan wizard Verification). Do NOT use for counting findings or general chat.",
        )
        .preamble(&worker_preamble("AnalyzeEndpointAgent", "analyze_endpoint"))
        .temperature(0.1)
        .max_tokens(512)
        .default_max_turns(WORKER_MAX_TURNS)
        .tool(AnalyzeEndpointRigTool {
            ctx,
            llm,
            state,
        })
        .build()
}

/// AttackPlan specialist agent (agent-as-tool for Yazg).
pub fn build_attack_plan_worker(
    llm: Arc<dyn PlannerLlm>,
    ctx: Arc<YazgSpecialistContext>,
    state: SharedYazgRigState,
) -> Agent<YazgRigModel> {
    AgentBuilder::new(YazgRigModel::new(llm.clone()))
        .name("attack_plan")
        .description(
            "Build an attack plan for a verified bound target (AttackPlanAgent). \
             Requires verified=true (or a bound verified target).",
        )
        .preamble(&worker_preamble("AttackPlanAgent", "attack_plan"))
        .temperature(0.1)
        .max_tokens(512)
        .default_max_turns(WORKER_MAX_TURNS)
        .tool(AttackPlanRigTool {
            ctx,
            llm,
            state,
        })
        .build()
}

/// GeneratePrompt specialist agent (agent-as-tool for Yazg).
pub fn build_generate_prompt_worker(
    llm: Arc<dyn PlannerLlm>,
    ctx: Arc<YazgSpecialistContext>,
    state: SharedYazgRigState,
) -> Agent<YazgRigModel> {
    AgentBuilder::new(YazgRigModel::new(llm.clone()))
        .name("generate_prompt")
        .description(
            "Attack Factory: invent a novel technique probe (GeneratePromptAgent). \
             Use only when factory_prompt_ready=true. Does not require a scan target.",
        )
        .preamble(&worker_preamble("GeneratePromptAgent", "generate_prompt"))
        .temperature(0.2)
        .max_tokens(768)
        .default_max_turns(WORKER_MAX_TURNS)
        .tool(GeneratePromptRigTool {
            ctx,
            llm,
            state,
        })
        .build()
}

/// Recommend specialist agent (agent-as-tool for Yazg).
pub fn build_recommend_worker(
    llm: Arc<dyn PlannerLlm>,
    ctx: Arc<YazgSpecialistContext>,
    state: SharedYazgRigState,
) -> Agent<YazgRigModel> {
    AgentBuilder::new(YazgRigModel::new(llm.clone()))
        .name("recommend")
        .description(
            "Post-scan remediation recommendations from completed attack results \
             (RecommendAgent). Requires attack_results_ready=true.",
        )
        .preamble(&worker_preamble("RecommendAgent", "recommend"))
        .temperature(0.2)
        .max_tokens(768)
        .default_max_turns(WORKER_MAX_TURNS)
        .tool(RecommendRigTool {
            ctx,
            llm,
            state,
        })
        .build()
}

/// Summary specialist agent (agent-as-tool for Yazg).
pub fn build_summary_worker(
    llm: Arc<dyn PlannerLlm>,
    ctx: Arc<YazgSpecialistContext>,
    state: SharedYazgRigState,
) -> Agent<YazgRigModel> {
    AgentBuilder::new(YazgRigModel::new(llm.clone()))
        .name("summary")
        .description(
            "Project or scan posture overview + highlights (SummaryAgent). \
             Requires summary_ready=true.",
        )
        .preamble(&worker_preamble("SummaryAgent", "summary"))
        .temperature(0.2)
        .max_tokens(768)
        .default_max_turns(WORKER_MAX_TURNS)
        .tool(SummaryRigTool {
            ctx,
            llm,
            state,
        })
        .build()
}

/// JudgeCoordinator specialist agent (agent-as-tool for Yazg).
///
/// Internally still runs JudgeCoordinator Rig orchestration (role worker tools).
pub fn build_judge_worker(
    orchestrator: Arc<dyn PlannerLlm>,
    ctx: Arc<YazgSpecialistContext>,
    state: SharedYazgRigState,
) -> Agent<YazgRigModel> {
    AgentBuilder::new(YazgRigModel::new(orchestrator.clone()))
        .name("judge")
        .description(
            "Consensus judging via JudgeCoordinatorAgent (JudgeWorker + ClassifierWorker + \
             AttackerWorker) orchestrated by Rig. Requires judge_ready=true and probe/response context.",
        )
        .preamble(&worker_preamble("JudgeCoordinatorAgent", "judge"))
        .temperature(0.1)
        .max_tokens(512)
        .default_max_turns(WORKER_MAX_TURNS)
        .tool(JudgeRigTool {
            ctx,
            orchestrator,
            state,
        })
        .build()
}
