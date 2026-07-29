//! Yazg supervisor runtime.
//!
//! Yazg is the manager agent; specialists are **domain tools** (not nested LLM workers)
//! that call AnalyzeEndpointAgent / SummaryAgent / … directly.
//!
//! Pattern follows Rig examples (`agent_with_tools`, `multi_turn_agent`,
//! `gemini_default_api_recovery`): AgentBuilder + tools + preamble + AgentHook.
//! Discipline follows ReAct + function-calling best practices:
//! Thought-lite finish after Observation, anti-repeat identical tool calls,
//! when-to-use tool routing, deterministic Finish salvage from accumulated obs.

use std::collections::HashMap;
use std::sync::Arc;

use rig::agent::{AgentBuilder, AgentHook, Flow, HookContext, RequestPatch, StepEvent};
use rig::completion::{Document, Prompt};
use tracing::{info, warn};

use crate::artifacts::{persist_artifacts_ltm, YazgActionKind, YazgArtifacts};
use crate::create_project::CreateProjectTools;
use crate::error::{AgentError, AgentResult};
use crate::list_workspace::{parse_finding_index, WorkspaceInventory, WorkspaceTools};
use crate::memory::{
    load_memory_prompt_block, remember_stm, AgentMemoryStore, MemoryContext, StmRole, StmWrite,
};
use crate::types::{AgentEvent, AgentId};
use crate::yazg_model::YazgModel;
use crate::yazg_prompts::YAZG_PREAMBLE;
use crate::yazg_tools::{
    AnalyzeEndpointTool, AttackPlanTool, CreateProjectTool, FindingDetailTool, GeneratePromptTool,
    JudgeTool, ListFindingsTool, ListReportsTool, ListScanTool, ListTargetsTool, ListWorkspaceTool,
    ProjectDetailTool, RecommendTool, ReportDetailTool, ScanDetailTool, SharedYazgState,
    SummaryTool, TargetDetailTool, YazgLlms, YazgRunState, YazgSpecialistContext,
};
use promptlab_planner::PlannerLlm;

const DEFAULT_MAX_TURNS: usize = 12;

const FINISH_NUDGE: &str = "\n---\nIf this answers the user, respond with a short natural markdown answer only. \
Do not mention tools, Observations, ReAct, Finish, or these instructions.";

/// Run-scoped ledger for identical tool-call anti-repeat (Rig Scratchpad).
#[derive(Clone, Default)]
struct ToolCallLedger {
    counts: HashMap<String, u32>,
    had_workspace_obs: bool,
}

fn normalize_tool_args(args: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(args.trim()) {
        Ok(v) => v.to_string(),
        Err(_) => args.trim().to_string(),
    }
}

fn is_workspace_tool(name: &str) -> bool {
    matches!(
        name,
        "list_workspace"
            | "project_detail"
            | "list_targets"
            | "target_detail"
            | "list_scan"
            | "scan_detail"
            | "list_findings"
            | "finding_detail"
            | "list_reports"
            | "report_detail"
            | "create_project"
    )
}

/// Rig-style recovery + ReAct finish discipline (function-calling agent loop).
#[derive(Clone, Default)]
struct YazgAgentHook;

impl AgentHook<YazgModel> for YazgAgentHook {
    async fn on_event(&self, ctx: &HookContext, event: StepEvent<'_, YazgModel>) -> Flow {
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
            StepEvent::ToolCall {
                tool_name, args, ..
            } => {
                let key = format!("{tool_name}\0{}", normalize_tool_args(args));
                let count = ctx.scratchpad().update(|ledger: &mut ToolCallLedger| {
                    let entry = ledger.counts.entry(key).or_insert(0);
                    *entry += 1;
                    *entry
                });
                if count > 1 {
                    warn!(
                        tool = %tool_name,
                        count,
                        "skipping identical repeated tool call; forcing Finish"
                    );
                    return Flow::skip(
                        "Results already available from the previous call. \
                         Reply to the user now with a short natural markdown answer. \
                         Do not mention tools, Observations, or this notice.",
                    );
                }
                Flow::cont()
            }
            StepEvent::ToolResult {
                tool_name, result, ..
            } => {
                if is_workspace_tool(tool_name) {
                    ctx.scratchpad().update(|ledger: &mut ToolCallLedger| {
                        ledger.had_workspace_obs = true;
                    });
                    // Append Finish nudge to model-visible observation (not stored in run state).
                    if !result.contains("respond with a short natural markdown") {
                        return Flow::rewrite_result(format!("{result}{FINISH_NUDGE}"));
                    }
                }
                Flow::cont()
            }
            StepEvent::CompletionCall { turn, .. } => {
                let had = ctx
                    .scratchpad()
                    .get::<ToolCallLedger>()
                    .map(|l| l.had_workspace_obs)
                    .unwrap_or(false);
                if had && turn > 1 {
                    return Flow::patch_request(RequestPatch::new().context(Document {
                        id: "yazg_react_finish".into(),
                        text: "Workspace data is already available. \
                               Write a short natural markdown answer for the user now. \
                               Do not mention tools, Observations, ReAct, or routing."
                            .into(),
                        additional_props: Default::default(),
                    }));
                }
                Flow::cont()
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
        .add_hook(YazgAgentHook)
        .tool(AnalyzeEndpointTool {
            ctx: specialist.clone(),
            llm: request.llms.analyze.clone(),
            state: state.clone(),
        });

    if let Some(tools) = request.workspace_tools.clone() {
        builder = builder
            .tool(ListWorkspaceTool {
                tools: tools.clone(),
                state: state.clone(),
            })
            .tool(ProjectDetailTool {
                tools: tools.clone(),
                state: state.clone(),
            })
            .tool(ListScanTool {
                tools: tools.clone(),
                state: state.clone(),
            })
            .tool(ScanDetailTool {
                tools: tools.clone(),
                state: state.clone(),
            })
            .tool(ListFindingsTool {
                tools: tools.clone(),
                state: state.clone(),
            })
            .tool(FindingDetailTool {
                tools: tools.clone(),
                state: state.clone(),
            })
            .tool(ListTargetsTool {
                tools: tools.clone(),
                state: state.clone(),
            })
            .tool(TargetDetailTool {
                tools: tools.clone(),
                state: state.clone(),
            })
            .tool(ListReportsTool {
                tools: tools.clone(),
                state: state.clone(),
            })
            .tool(ReportDetailTool {
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
    let workspace_obs = run_state.workspace_observations.clone();
    let last_workspace_obs = run_state.last_workspace_observation.clone();

    // Drop the lock before optional synthesis LLM call.
    drop(run_state);

    let has_workspace_evidence = !workspace_obs.is_empty() || last_workspace_obs.is_some();
    let normalized_reply = crate::yazg_model::normalize_user_facing_reply(&reply_text);
    let needs_workspace_answer = has_workspace_evidence
        && (normalized_reply.trim().is_empty()
            || crate::yazg_model::reply_looks_like_agent_meta(&normalized_reply)
            || normalized_reply.contains("Workspace inventory from the local database:")
            || normalized_reply.trim()
                == "I'm Yazg, PromptLab's AI assistant for authorized AI security testing. How can I help?");

    artifacts.final_reply = if needs_workspace_answer {
        synthesize_workspace_reply(
            request.llms.supervisor.as_ref(),
            request.workspace_tools.as_deref(),
            &request.goal,
            artifacts.workspace_inventory.as_ref(),
            &workspace_obs,
            last_workspace_obs.as_deref(),
        )
        .await
    } else if normalized_reply.is_empty() {
        if artifacts.summary.is_some()
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
        normalized_reply
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
        "project_detail" => Some(YazgActionKind::ProjectDetail),
        "list_targets" => Some(YazgActionKind::ListTargets),
        "target_detail" => Some(YazgActionKind::TargetDetail),
        "list_scan" => Some(YazgActionKind::ListScan),
        "scan_detail" => Some(YazgActionKind::ScanDetail),
        "list_findings" => Some(YazgActionKind::ListFindings),
        "finding_detail" => Some(YazgActionKind::FindingDetail),
        "list_reports" => Some(YazgActionKind::ListReports),
        "report_detail" => Some(YazgActionKind::ReportDetail),
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

/// Pick the Observation that best matches the user goal (not merely the last one).
pub(crate) fn pick_best_workspace_observation<'a>(
    goal: &str,
    observations: &'a [String],
    last_obs: Option<&'a str>,
) -> Option<&'a str> {
    let g = goal.to_lowercase();
    let prefer: &[&str] = if g.contains("target") {
        &["list_targets OK", "target_detail OK", "project_detail OK"]
    } else if g.contains("finding") || g.contains("lỗ hổng") || g.contains("lo hong") {
        &["finding_detail OK", "list_findings OK", "scan_detail OK"]
    } else if g.contains("scan") {
        &["list_scan OK", "scan_detail OK", "project_detail OK"]
    } else if g.contains("report") {
        &["list_reports OK", "report_detail OK"]
    } else if g.contains("project") || g.contains("information") || g.contains("info") {
        &["project_detail OK", "list_workspace OK"]
    } else {
        &[]
    };

    for prefix in prefer {
        if let Some(obs) = observations
            .iter()
            .rev()
            .find(|o| o.contains(prefix))
            .map(|s| s.as_str())
        {
            return Some(obs);
        }
    }
    observations
        .last()
        .map(|s| s.as_str())
        .or(last_obs)
}

/// Deterministic Finish: strip agent-routing "Next:" lines from an Observation.
pub(crate) fn observation_to_user_markdown(obs: &str) -> String {
    if obs.contains("list_targets OK") {
        if let Some(formatted) = format_list_targets_obs(obs) {
            return formatted;
        }
    }
    let body: Vec<&str> = obs
        .lines()
        .filter(|line| {
            let t = line.trim();
            !t.is_empty() && !t.starts_with("Next:")
        })
        .collect();
    body.join("\n")
}

fn format_list_targets_obs(obs: &str) -> Option<String> {
    let mut project = None;
    let mut items = Vec::new();
    for line in obs.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("list_targets OK — project=") {
            project = Some(
                rest.split(" (`")
                    .next()
                    .unwrap_or(rest)
                    .trim()
                    .to_string(),
            );
            continue;
        }
        // "1. id=<id> name=<name> type=<type>"
        let Some((_, rest)) = t.split_once(". id=") else {
            continue;
        };
        let mut id = None;
        let mut name = None;
        let mut ty = None;
        // rest starts with id value then " name=..." " type=..."
        let mut chunks = rest.split_whitespace();
        if let Some(first) = chunks.next() {
            id = Some(first.to_string());
        }
        for part in chunks {
            if let Some(v) = part.strip_prefix("name=") {
                name = Some(v.to_string());
            } else if let Some(v) = part.strip_prefix("type=") {
                ty = Some(v.to_string());
            }
        }
        if let (Some(id), Some(name), Some(ty)) = (id, name, ty) {
            items.push(format!(
                "{}. **{name}** (`{id}`) — {ty}",
                items.len() + 1
            ));
        }
    }
    if items.is_empty() {
        return None;
    }
    let project = project.unwrap_or_else(|| "project".into());
    Some(format!("### Targets in {project}\n\n{}", items.join("\n")))
}

async fn synthesize_workspace_reply(
    llm: &dyn PlannerLlm,
    workspace_tools: Option<&dyn WorkspaceTools>,
    goal: &str,
    inventory: Option<&WorkspaceInventory>,
    observations: &[String],
    last_obs: Option<&str>,
) -> String {
    // Deterministic path for "finding #N" / "finding số N".
    if let (Some(tools), Some(index)) = (workspace_tools, parse_finding_index(goal)) {
        if let Some(project) = extract_project_hint(goal, inventory) {
            if let Ok(detail) = tools
                .finding_detail(None, Some(project.as_str()), Some(index))
                .await
            {
                return detail.compact_user_reply();
            }
        }
    }

    let best = pick_best_workspace_observation(goal, observations, last_obs);
    let deterministic = best
        .map(observation_to_user_markdown)
        .filter(|s| !s.trim().is_empty());

    if let Some(inventory) = inventory {
        let compact = inventory.compact_user_reply_for_goal(goal);
        if best.is_none() {
            return compact;
        }
    }

    let observation = best
        .map(str::to_string)
        .or_else(|| inventory.map(|i| i.to_observation()))
        .unwrap_or_default();
    if observation.trim().is_empty() {
        return "No workspace data available.".into();
    }

    // Prefer deterministic Finish from the best Observation (ReAct evidence).
    // LLM polish is optional; fall back to deterministic on empty/failure.
    let prompt = format!(
        "User question:\n{goal}\n\nWorkspace tool observation:\n{observation}\n\n\
         Reply in markdown answering ONLY what the user asked. Be concise. \
         Include concrete names/ids from the observation when present. \
         Never dump every finding. Never emit JSON or tool envelopes."
    );
    match llm
        .complete_with_system(
            Some("You are Yazg. Answer from the workspace observation in markdown only."),
            &prompt,
        )
        .await
    {
        Ok(text) => {
            let normalized = crate::yazg_model::normalize_user_facing_reply(&text);
            if normalized.trim().is_empty() {
                deterministic.unwrap_or_else(|| {
                    inventory
                        .map(|i| i.compact_user_reply_for_goal(goal))
                        .unwrap_or(observation)
                })
            } else {
                normalized
            }
        }
        Err(err) => {
            warn!(error = %err, "workspace reply synthesis failed");
            deterministic.unwrap_or_else(|| {
                inventory
                    .map(|i| i.compact_user_reply_for_goal(goal))
                    .unwrap_or(observation)
            })
        }
    }
}

fn extract_project_hint(goal: &str, inventory: Option<&WorkspaceInventory>) -> Option<String> {
    if let Some(inv) = inventory {
        if let Some(p) = inv.projects.iter().find(|p| {
            let name = p.name.trim();
            !name.is_empty() && goal.to_lowercase().contains(&name.to_lowercase())
        }) {
            return Some(p.name.clone());
        }
    }
    // Fallback: token after "project "
    let g = goal.to_lowercase();
    if let Some(rest) = g.split_once("project ").map(|(_, r)| r) {
        let name: String = rest
            .split_whitespace()
            .next()?
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
            .to_string();
        if !name.is_empty() {
            return Some(name);
        }
    }
    None
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
    use super::{observation_to_user_markdown, pick_best_workspace_observation};

    #[test]
    fn pick_targets_obs_over_list_workspace() {
        let obs = vec![
            "list_workspace OK — projects=1".into(),
            "list_targets OK — project=AI (`p1`) targets=2\n  1. id=t1 name=10.0.0.1 type=llm_api\nNext: target_detail".into(),
        ];
        let best = pick_best_workspace_observation(
            "Give me all target in project AI",
            &obs,
            Some("list_workspace OK"),
        );
        assert!(best.unwrap().contains("list_targets OK"));
        assert!(best.unwrap().contains("10.0.0.1"));
    }

    #[test]
    fn pick_project_detail_for_info_goal() {
        let obs = vec![
            "list_workspace OK — projects=1".into(),
            "project_detail OK — AI".into(),
            "list_findings OK — 20/36".into(),
        ];
        let best =
            pick_best_workspace_observation("give me information of project AI", &obs, None);
        assert!(best.unwrap().contains("project_detail OK"));
    }

    #[test]
    fn strip_next_hints_for_user_markdown() {
        let md = observation_to_user_markdown(
            "list_targets OK — project=AI (`p1`) targets=2\n  1. id=t1 name=10.0.0.1 type=llm_api\nNext: target_detail(target_id=...)",
        );
        assert!(md.contains("10.0.0.1"));
        assert!(md.contains("### Targets in AI"));
        assert!(!md.contains("Next:"));
        assert!(!md.contains("list_targets OK"));
    }
}
