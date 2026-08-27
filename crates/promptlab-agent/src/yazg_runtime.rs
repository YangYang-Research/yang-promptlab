//! Yazg supervisor runtime.
//!
//! Yazg is the manager agent; specialists are **domain tools** (not nested LLM workers)
//! that call AnalyzeEndpointAgent / SummaryAgent / … directly.
//!
//! Pattern follows Rig examples (`agent_with_tools`, `multi_turn_agent`,
//! `gemini_default_api_recovery`): AgentBuilder + tools + preamble + AgentHook.
//! Discipline follows ReAct + function-calling best practices:
//! decide tool-vs-text each turn, ignore irrelevant Observations, anti-repeat
//! identical tool calls, LLM-judged Finish salvage (no chat keyword hardcoding).

use std::collections::HashMap;
use std::sync::Arc;

use rig::agent::{AgentBuilder, AgentHook, Flow, HookContext, RequestPatch, StepEvent};
use rig::completion::{AssistantContent, Message, Prompt};
use rig::message::{ToolChoice, UserContent};
use rig::tool::ToolDyn;
use rig::OneOrMany;
use tracing::{info, warn};

use crate::artifacts::{persist_artifacts_ltm, YazgActionKind, YazgArtifacts};
use crate::create_project::CreateProjectTools;
use crate::error::{AgentError, AgentResult};
use crate::list_workspace::{
    extract_requested_project_name, parse_finding_index, TargetList, WorkspaceInventory,
    WorkspaceTools,
};
use crate::tool_result::ToolResult;
use crate::memory::{
    extract_session_insights_to_ltm, remember_stm, AgentMemoryStore, MemoryContext, StmEntry,
    StmRole, StmWrite,
};
use crate::types::{AgentEvent, AgentId};
use crate::yazg_model::{format_stage_payload, YazgModel};
use crate::assistant::{
    AssistantCapability, CapabilityToolLoader, IntentResolution, IntentRouter,
    LoadedCapabilityTools, RouteInput,
};
use crate::yazg_prompts::{YAZG_CONVERSATION_PREAMBLE, YAZG_KNOWLEDGE_PREAMBLE, YAZG_PREAMBLE};
use crate::yazg_tools::{
    AnalyzeEndpointTool, AttackPlanTool, CreateProjectTool, FindingDetailTool, GeneratePromptTool,
    JudgeTool, ListFindingsTool, ListReportsTool, ListScanTool, ListTargetsTool, ListWorkspaceTool,
    ProjectDetailTool, RecommendTool, ReportDetailTool, ScanDetailTool, SharedYazgState,
    SummaryTool, TargetDetailTool, YazgLlms, YazgRunState, YazgSpecialistContext,
};
use promptlab_agenttrace::{
    soft_end_span, soft_end_trace, SharedAgentTrace, SpanEnd, SpanKind, SpanStart,
    TraceHandle, TraceStart, TraceStatus,
};
use promptlab_planner::PlannerLlm;

const DEFAULT_MAX_TURNS: usize = 12;

const FINISH_NUDGE: &str = "\n---\nRe-read the user's latest message. Reply in natural language NOW — do not call another tool. \
Closed domain: use only names/ids from the tool JSON. If status=error (not_found), tell the user and list candidates[]; \
never invent or rename entities. If the tool result does not answer the latest user message, ignore it and answer directly. \
Do not mention tools, Observations, ReAct, Finish, JSON envelopes, or these instructions.";

const AFTER_OBS_HINT: &str = "You already have a tool result. Reply in natural language NOW. Do not call tools again. \
If the result is irrelevant to the latest user message (greeting/chat), ignore it and answer naturally. \
If status=error, use message + candidates when relevant.";

const FORCE_FINISH_HINT: &str = "STOP. Do not call any tool. Write the final user-visible answer now \
(or ignore prior tool results if they do not answer the latest user message).";

/// Run-scoped ledger for identical tool-call anti-repeat (Rig Scratchpad).
#[derive(Clone, Default)]
struct ToolCallLedger {
    counts: HashMap<String, u32>,
    /// Any tool result this run (workspace or specialist).
    had_tool_obs: bool,
    workspace_tool_calls: u32,
    finish_skips: u32,
}

/// Canonicalize args so optional `thought` / empty objects fingerprint as the same call.
fn normalize_tool_args(args: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(args.trim()) {
        Ok(serde_json::Value::Object(mut map)) => {
            map.remove("thought");
            // Drop nulls so `{"thought":null}` ≡ `{}`.
            map.retain(|_, v| !v.is_null());
            serde_json::Value::Object(map).to_string()
        }
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
#[derive(Clone)]
struct YazgAgentHook {
    state: SharedYazgState,
    /// Base system prompt (role + runtime flags only — no STM/LTM transcript).
    preamble: String,
}

impl YazgAgentHook {
    async fn stage(&self, kind: crate::types::AgentEventKind, message: impl Into<String>) {
        let message = message.into();
        let conversation_id = self.state.lock().await.conversation_id.clone();
        let event = AgentEvent::emit_kind(AgentId::Yazg, kind, message, conversation_id);
        let mut guard = self.state.lock().await;
        guard.artifacts.events.push(event);
    }

    async fn stage_json(
        &self,
        kind: crate::types::AgentEventKind,
        stage: &str,
        body: serde_json::Value,
    ) {
        self.stage(kind, format_stage_payload(stage, body)).await;
    }

    async fn stm_event(
        &self,
        role: StmRole,
        memory_key: Option<String>,
        content: String,
        content_json: Option<serde_json::Value>,
        importance: f64,
    ) {
        let (memory, memory_ctx) = {
            let guard = self.state.lock().await;
            (guard.memory.clone(), guard.memory_ctx.clone())
        };
        remember_stm(
            memory.as_deref(),
            &memory_ctx,
            StmWrite {
                agent_id: AgentId::Yazg,
                role,
                memory_key,
                content,
                content_json,
                importance,
            },
        )
        .await;
    }

    fn preamble_with(&self, extra: &str) -> String {
        format!("{}\n\n{extra}", self.preamble.trim_end())
    }
}

impl AgentHook<YazgModel> for YazgAgentHook {
    async fn on_event(&self, ctx: &HookContext, event: StepEvent<'_, YazgModel>) -> Flow {
        match event {
            StepEvent::InvalidToolCall(inv) => {
                warn!(tool = %inv.tool_name, "invalid tool call; asking model to reply in text");
                self.stage_json(
                    crate::types::AgentEventKind::React,
                    "invalid_tool",
                    serde_json::json!({
                        "tool": inv.tool_name,
                        "available_tools": inv.available_tools,
                    }),
                )
                .await;
                Flow::retry(format!(
                    "`{}` is not a valid tool. Available tools: [{}]. \
                     Reply to the user in plain text — do not invent tools.",
                    inv.tool_name,
                    inv.available_tools.join(", ")
                ))
            }
            StepEvent::ToolCall {
                tool_name,
                args,
                tool_call_id,
                internal_call_id,
                ..
            } => {
                let key = format!("{tool_name}\0{}", normalize_tool_args(args));
                let (count, had, workspace_calls, finish_skips) =
                    ctx.scratchpad().update(|ledger: &mut ToolCallLedger| {
                        let entry = ledger.counts.entry(key).or_insert(0);
                        *entry += 1;
                        let count = *entry;
                        if is_workspace_tool(tool_name) {
                            ledger.workspace_tool_calls =
                                ledger.workspace_tool_calls.saturating_add(1);
                        }
                        (
                            count,
                            ledger.had_tool_obs,
                            ledger.workspace_tool_calls,
                            ledger.finish_skips,
                        )
                    });
                let args_json: serde_json::Value =
                    serde_json::from_str(args).unwrap_or_else(|_| serde_json::json!(args));
                {
                    let mut guard = self.state.lock().await;
                    if !guard.tools_used.iter().any(|t| t == tool_name) {
                        guard.tools_used.push(tool_name.to_string());
                    }
                }
                self.stm_event(
                    StmRole::Tool,
                    Some(tool_name.to_string()),
                    format!("tool_call:{tool_name}"),
                    Some(serde_json::json!({
                        "tool": tool_name,
                        "tool_call_id": tool_call_id,
                        "args": args_json,
                    })),
                    0.55,
                )
                .await;
                self.stage_json(
                    crate::types::AgentEventKind::ToolCall,
                    "tool_call_request",
                    serde_json::json!({
                        "tool": tool_name,
                        "tool_call_id": tool_call_id,
                        "internal_call_id": internal_call_id,
                        "args": args_json,
                        "count": count,
                        "had_tool_obs": had,
                    }),
                )
                .await;
                {
                    let guard = self.state.lock().await;
                    let trace = guard.active_trace.clone();
                    let parent = guard
                        .active_llm_span
                        .as_ref()
                        .map(|s| s.id().to_string())
                        .or_else(|| guard.parent_llm_span_id.clone());
                    drop(guard);
                    let span = promptlab_agenttrace::start_span!(
                        trace.as_ref(),
                        SpanStart {
                            name: format!("tool:{tool_name}"),
                            kind: SpanKind::Tool,
                            parent_span_id: parent,
                            inputs: Some(serde_json::json!({
                                "tool": tool_name,
                                "tool_call_id": tool_call_id,
                                "args": args_json,
                            })),
                            attributes: std::collections::BTreeMap::from([(
                                "tool".into(),
                                tool_name.to_string(),
                            )]),
                        }
                    )
                    .await;
                    if let Some(span) = span {
                        let key = tool_call_id
                            .unwrap_or(internal_call_id)
                            .to_string();
                        self.state
                            .lock()
                            .await
                            .active_tool_spans
                            .insert(key, span);
                    }
                }
                if count > 1 {
                    warn!(
                        tool = %tool_name,
                        count,
                        finish_skips,
                        "repeat tool call; forcing Finish"
                    );
                    let skips = ctx.scratchpad().update(|ledger: &mut ToolCallLedger| {
                        ledger.finish_skips = ledger.finish_skips.saturating_add(1);
                        ledger.finish_skips
                    });
                    self.stage_json(
                        crate::types::AgentEventKind::React,
                        "hook_skip_repeat",
                        serde_json::json!({
                            "tool": tool_name,
                            "finish_skips": skips,
                            "count": count,
                        }),
                    )
                    .await;
                    // After two forced skips the model is thrashing — end the loop;
                    // run_yazg salvages a natural-language reply.
                    if skips >= 2 {
                        self.stage_json(
                            crate::types::AgentEventKind::React,
                            "hook_terminate",
                            serde_json::json!({ "reason": "repeat_tool_thrash" }),
                        )
                        .await;
                        return Flow::terminate(
                            "force_finish_after_repeat_tool: model kept calling tools after Finish nudge",
                        );
                    }
                    return Flow::skip(
                        "That tool result is already available. \
                         Write the final user-visible answer NOW in plain text. \
                         Do not call tools again.",
                    );
                }
                // After any prior tool result, block further tool thrash (especially chat).
                if had {
                    if !is_workspace_tool(tool_name)
                        || tool_name == "list_workspace"
                        || workspace_calls > 2
                    {
                        warn!(
                            tool = %tool_name,
                            workspace_calls,
                            "skipping extra tool after Observation; forcing Finish"
                        );
                        let skips = ctx.scratchpad().update(|ledger: &mut ToolCallLedger| {
                            ledger.finish_skips = ledger.finish_skips.saturating_add(1);
                            ledger.finish_skips
                        });
                        self.stage_json(
                            crate::types::AgentEventKind::React,
                            "hook_skip_after_obs",
                            serde_json::json!({
                                "tool": tool_name,
                                "finish_skips": skips,
                                "workspace_calls": workspace_calls,
                            }),
                        )
                        .await;
                        if skips >= 2 {
                            self.stage_json(
                                crate::types::AgentEventKind::React,
                                "hook_terminate",
                                serde_json::json!({ "reason": "tool_after_obs_thrash" }),
                            )
                            .await;
                            return Flow::terminate(
                                "force_finish_after_obs: model called another tool instead of answering",
                            );
                        }
                        return Flow::skip(
                            "You already have a tool result. Reply to the user now in plain text. \
                             Do not call more tools.",
                        );
                    }
                }
                Flow::cont()
            }
            StepEvent::ToolResult {
                tool_name,
                result,
                args,
                tool_call_id,
                internal_call_id,
                ..
            } => {
                ctx.scratchpad().update(|ledger: &mut ToolCallLedger| {
                    ledger.had_tool_obs = true;
                });
                let args_json: serde_json::Value =
                    serde_json::from_str(args).unwrap_or_else(|_| serde_json::json!(args));
                let result_json: serde_json::Value = serde_json::from_str(result)
                    .unwrap_or_else(|_| serde_json::json!(result));
                self.stm_event(
                    StmRole::Observation,
                    Some(tool_name.to_string()),
                    truncate(result, 4_000),
                    Some(serde_json::json!({
                        "event": "tool_result",
                        "tool": tool_name,
                        "tool_call_id": tool_call_id,
                        "request_args": args_json,
                        "response_preview": truncate(result, 2_000),
                    })),
                    0.6,
                )
                .await;
                self.stage_json(
                    crate::types::AgentEventKind::React,
                    "tool_result_response",
                    serde_json::json!({
                        "tool": tool_name,
                        "tool_call_id": tool_call_id,
                        "internal_call_id": internal_call_id,
                        "request_args": args_json,
                        "response": result_json,
                    }),
                )
                .await;
                {
                    let key = tool_call_id
                        .unwrap_or(internal_call_id)
                        .to_string();
                    let span = self.state.lock().await.active_tool_spans.remove(&key);
                    soft_end_span(
                        span.as_ref(),
                        SpanEnd {
                            inputs: None,
                            outputs: Some(result_json),
                            status: TraceStatus::Ok,
                            metrics: std::collections::BTreeMap::new(),
                            attributes: std::collections::BTreeMap::new(),
                        },
                    )
                    .await;
                }
                // Append Finish nudge for every tool (workspace + specialist).
                if !result.contains("Reply in natural language NOW")
                    && !result.contains("Write the final user-visible answer NOW")
                {
                    return Flow::rewrite_result(format!("{result}{FINISH_NUDGE}"));
                }
                Flow::cont()
            }
            StepEvent::CompletionCall {
                turn,
                prompt,
                history,
                ..
            } => {
                let had = ctx
                    .scratchpad()
                    .get::<ToolCallLedger>()
                    .map(|l| l.had_tool_obs)
                    .unwrap_or(false);
                self.stage_json(
                    crate::types::AgentEventKind::Llm,
                    "completion_call",
                    serde_json::json!({
                        "turn": turn,
                        "had_tool_obs": had,
                        "prompt": prompt,
                        "history": history,
                    }),
                )
                .await;
                if had {
                    let text = if turn > 2 {
                        FORCE_FINISH_HINT
                    } else {
                        AFTER_OBS_HINT
                    };
                    // Append finish discipline to system — never as a fake user Document
                    // (that caused models to answer the hint with "false"/"yes").
                    return Flow::patch_request(
                        RequestPatch::new().preamble(self.preamble_with(text)),
                    );
                }
                Flow::cont()
            }
            StepEvent::CompletionResponse { prompt, response } => {
                self.stage_json(
                    crate::types::AgentEventKind::Llm,
                    "completion_response",
                    serde_json::json!({
                        "prompt": prompt,
                        "choice": response.choice,
                        "usage": {
                            "input_tokens": response.usage.input_tokens,
                            "output_tokens": response.usage.output_tokens,
                        },
                        "raw_response": response.raw_response,
                    }),
                )
                .await;
                Flow::cont()
            }
            StepEvent::ModelTurnFinished { turn, content, usage } => {
                self.stage_json(
                    crate::types::AgentEventKind::Llm,
                    "model_turn_finished",
                    serde_json::json!({
                        "turn": turn,
                        "content": content,
                        "usage": {
                            "input_tokens": usage.input_tokens,
                            "output_tokens": usage.output_tokens,
                        },
                    }),
                )
                .await;
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
    /// Optional AgentTrace store for tracing.
    pub agent_trace: Option<SharedAgentTrace>,
    /// Display name of the active inference model (written onto AgentTrace spans).
    pub model_label: Option<String>,
    /// Skip capability router and bind this capability's tools (or none for Conversation).
    pub forced_capability: Option<crate::assistant::capability_registry::AssistantCapability>,
    /// Do not append the goal as a user STM row (internal follow-up turns).
    pub skip_user_stm: bool,
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
            agent_trace: None,
            model_label: None,
            forced_capability: None,
            skip_user_stm: false,
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

    pub fn with_agent_trace(mut self, store: Option<SharedAgentTrace>) -> Self {
        self.agent_trace = store;
        self
    }

    pub fn with_model_label(mut self, model_label: Option<String>) -> Self {
        self.model_label = model_label
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
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

    pub fn with_forced_capability(
        mut self,
        capability: crate::assistant::capability_registry::AssistantCapability,
    ) -> Self {
        self.forced_capability = Some(capability);
        self
    }

    pub fn with_skip_user_stm(mut self, skip: bool) -> Self {
        self.skip_user_stm = skip;
        self
    }
}

/// Run Yazg tool-calling loop and return artifacts + raw PromptResponse JSON.
pub async fn run_yazg(request: YazgRequest) -> AgentResult<(YazgArtifacts, serde_json::Value)> {
    let conversation_id = {
        let s = request.memory_ctx.session_id.trim();
        (!s.is_empty()).then(|| s.to_string())
    };

    // AgentTrace: one trace per Yazg turn (soft-fail).
    let active_trace = if let Some(flow) = request.agent_trace.as_ref() {
        match flow.experiment("yazg").await {
            Ok(exp) => {
                let mut tags = std::collections::BTreeMap::new();
                tags.insert("agent".into(), "yazg".into());
                if let Some(model) = request.model_label.as_deref() {
                    tags.insert("model".into(), model.to_string());
                }
                match exp
                    .start_trace(TraceStart {
                        name: "yazg_turn".into(),
                        session_id: conversation_id.clone(),
                        tags,
                    })
                    .await
                {
                    Ok(trace) => Some(trace),
                    Err(err) => {
                        warn!(error = %err, "agenttrace start_trace failed");
                        None
                    }
                }
            }
            Err(err) => {
                warn!(error = %err, "agenttrace experiment failed");
                None
            }
        }
    } else {
        None
    };

    let mut artifacts = YazgArtifacts::default();
    artifacts.trace_id = active_trace.as_ref().map(|t| t.id().to_string());
    artifacts.events.push(AgentEvent::emit_kind(
        AgentId::Yazg,
        crate::types::AgentEventKind::Started,
        "Yazg agent loop started",
        conversation_id.clone(),
    ));

    if !request.skip_user_stm {
        remember_stm(
            request.memory.as_deref(),
            &request.memory_ctx,
            StmWrite {
                agent_id: AgentId::Yazg,
                role: StmRole::User,
                memory_key: Some("message".into()),
                content: request.goal.clone(),
                content_json: Some(serde_json::json!({
                    "event": "user_message",
                    "session_id": request.memory_ctx.session_id,
                })),
                importance: 0.7,
            },
        )
        .await;
    }

    // STM → prior chat turns (messages[]), never into system prompt.
    // LTM conversation.* stays out of the prompt; durable facts are for retrieval, not transcript.
    let prior_history = load_prior_chat_history(
        request.memory.as_deref(),
        &request.memory_ctx.session_id,
        request.goal.trim(),
    )
    .await;

    // Capability routing: LLM #1 classifies from the latest user message only
    // (no chat history). Then loader injects that capability's tools into LLM #2.
    // Span must wrap the LLM call — start before resolve, end after.
    let classify_inputs = serde_json::json!({
        "latest_user_message": request.goal.trim(),
    });
    let classify_span = capability_classify(
        active_trace.as_ref(),
        classify_inputs.clone(),
        request.model_label.as_deref(),
    )
    .await;
    let route_started = std::time::Instant::now();

    let resolution = if let Some(cap) = request.forced_capability {
        IntentResolution {
            capability: cap,
            confidence: 1.0,
            reason: "forced_capability".into(),
            raw_classifier_output: None,
            classifier_request: Some(classify_inputs.clone()),
        }
    } else {
        IntentRouter::new()
            .resolve_with_llm(
                request.llms.supervisor.as_ref(),
                &RouteInput {
                    latest_user_message: request.goal.trim(),
                },
            )
            .await
    };

    let mut classify_inputs = resolution.classifier_request.clone().unwrap_or(classify_inputs);
    if let Some(model) = request.model_label.as_deref() {
        if let Some(obj) = classify_inputs.as_object_mut() {
            obj.insert("model".into(), serde_json::json!(model));
        }
    }
    let route_ms = route_started.elapsed().as_millis();

    artifacts.events.push(AgentEvent::emit_kind(
        AgentId::Yazg,
        crate::types::AgentEventKind::Llm,
        crate::yazg_model::format_stage_payload(
            "capability_classify_request",
            classify_inputs.clone(),
        ),
        conversation_id.clone(),
    ));

    let loaded = CapabilityToolLoader::new().load(&resolution);

    soft_end_span(
        classify_span.as_ref(),
        SpanEnd {
            // Replace the start-time stub with the full classifier wire request.
            inputs: Some(classify_inputs.clone()),
            outputs: Some(serde_json::json!({
                "capability": loaded.capability.as_str(),
                "confidence": (loaded.confidence * 100.0).round() / 100.0,
                "reason": loaded.reason,
                "content": resolution.raw_classifier_output,
            })),
            status: TraceStatus::Ok,
            metrics: std::collections::BTreeMap::from([
                ("latency_ms".into(), route_ms as f64),
                ("confidence".into(), loaded.confidence as f64),
            ]),
            attributes: std::collections::BTreeMap::from([(
                "capability".into(),
                loaded.capability.as_str().to_string(),
            )]),
        },
    )
    .await;

    artifacts.events.push(AgentEvent::emit_kind(
        AgentId::Yazg,
        crate::types::AgentEventKind::Llm,
        crate::yazg_model::format_stage_payload(
            "capability_classify_response",
            serde_json::json!({
                "content": resolution.raw_classifier_output,
                "capability": loaded.capability.as_str(),
                "confidence": (loaded.confidence * 100.0).round() / 100.0,
                "reason": loaded.reason,
                "tool_calls": [],
                "latency_ms": route_ms,
            }),
        ),
        conversation_id.clone(),
    ));

    info!(
        capability = %loaded.capability,
        tool_count = loaded.tool_count(),
        tools = ?loaded.tool_names,
        allow_tool_calling = loaded.allow_tool_calling,
        confidence = loaded.confidence,
        reason = %loaded.reason,
        latency_ms = route_ms,
        "yazg capability resolved (llm)"
    );
    artifacts.events.push(AgentEvent::emit_kind(
        AgentId::Yazg,
        crate::types::AgentEventKind::Info,
        format!(
            "capability={} tools={} names={:?} tool_used=pending reason={} confidence={:.2} latency_ms={} router=llm",
            loaded.capability,
            loaded.tool_count(),
            loaded.tool_names,
            loaded.reason,
            loaded.confidence,
            route_ms
        ),
        conversation_id.clone(),
    ));

    let state: SharedYazgState = Arc::new(tokio::sync::Mutex::new(YazgRunState {
        conversation_id: conversation_id.clone(),
        memory: request.memory.clone(),
        memory_ctx: request.memory_ctx.clone(),
        active_trace: active_trace.clone(),
        model_label: request.model_label.clone(),
        ..Default::default()
    }));
    let specialist = Arc::new(request.specialist.clone());
    let context_block = capability_context_block(loaded.capability, specialist.as_ref());
    let model = YazgModel::new(request.llms.supervisor.clone())
        .with_stage_sink(state.clone())
        .with_model_label(request.model_label.clone());

    let base_preamble = match loaded.capability {
        AssistantCapability::Conversation => YAZG_CONVERSATION_PREAMBLE,
        AssistantCapability::Knowledge => YAZG_KNOWLEDGE_PREAMBLE,
        _ => YAZG_PREAMBLE,
    };
    let preamble = {
        let mut p = base_preamble.to_string();
        if !context_block.trim().is_empty() {
            p.push_str("\n\n");
            p.push_str(context_block.trim());
        }
        if !loaded.system_addon.trim().is_empty() {
            p.push_str("\n\n");
            p.push_str(loaded.system_addon.trim());
        }
        p
    };

    let max_turns = if loaded.allow_tool_calling {
        request.max_turns
    } else {
        1
    };

    let tool_choice = if loaded.allow_tool_calling {
        ToolChoice::Auto
    } else {
        ToolChoice::None
    };

    let boxed_tools = build_capability_tools(&loaded, &request, &state, specialist.clone());

    let prompt_result = if boxed_tools.is_empty() {
        let agent = AgentBuilder::new(model)
            .name("Yazg")
            .description("PromptLab AI Assistant")
            .preamble(&preamble)
            .temperature(0.2)
            .max_tokens(1024)
            .default_max_turns(max_turns)
            .tool_choice(tool_choice)
            .add_hook(YazgAgentHook {
                state: state.clone(),
                preamble: preamble.clone(),
            })
            .build();
        info!(goal = %truncate(&request.goal, 120), "Yazg agent handle (no tools)");
        agent
            .prompt(request.goal.trim())
            .history(prior_history)
            .max_turns(max_turns)
            .extended_details()
            .await
    } else {
        let agent = AgentBuilder::new(model)
            .name("Yazg")
            .description("PromptLab AI Assistant")
            .preamble(&preamble)
            .temperature(0.2)
            .max_tokens(1024)
            .default_max_turns(max_turns)
            .tool_choice(tool_choice)
            .add_hook(YazgAgentHook {
                state: state.clone(),
                preamble: preamble.clone(),
            })
            .tools(boxed_tools)
            .build();
        info!(goal = %truncate(&request.goal, 120), "Yazg agent handle");
        agent
            .prompt(request.goal.trim())
            .history(prior_history)
            .max_turns(max_turns)
            .extended_details()
            .await
    };

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
            // MaxTurns thrash OR hook terminate after repeat tools — salvage a text reply.
            if msg.contains("MaxTurnsError")
                || msg.contains("force_finish")
                || msg.to_lowercase().contains("cancelled")
                || msg.to_lowercase().contains("prompt cancelled")
            {
                warn!(error = %err, "Yazg loop ended early; salvaging reply");
                (
                    String::new(),
                    serde_json::json!({
                        "error": if msg.contains("MaxTurnsError") { "max_turns" } else { "force_finish" },
                        "detail": msg
                    }),
                    format!("Yazg early-exit salvage: {msg}"),
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
    artifacts.pending_action = run_state.artifacts.pending_action.take();
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
    let tools_used = run_state.tools_used.clone();

    // Drop the lock before optional synthesis LLM call.
    drop(run_state);

    let has_workspace_evidence = !workspace_obs.is_empty() || last_workspace_obs.is_some();
    let normalized_reply = crate::yazg_model::normalize_user_facing_reply(&reply_text);
    let raw_workspace_dump = looks_like_raw_workspace_obs(&normalized_reply);
    // Salvage only when the loop failed to produce a usable user-facing reply.
    // Do NOT treat the empty-completion identity fallback as "needs workspace" —
    // that forced inventory answers for greetings after accidental tool calls.
    let needs_salvage = normalized_reply.trim().is_empty()
        || crate::yazg_model::reply_looks_like_agent_meta(&normalized_reply)
        || normalized_reply.contains("Workspace inventory from the local database:")
        || raw_workspace_dump;

    artifacts.events.push(AgentEvent::emit_kind(
        AgentId::Yazg,
        crate::types::AgentEventKind::React,
        format!(
            "stage=reply_normalize empty={} meta={} salvage={} tools_obs={}",
            normalized_reply.trim().is_empty(),
            crate::yazg_model::reply_looks_like_agent_meta(&normalized_reply),
            needs_salvage,
            workspace_obs.len()
        ),
        conversation_id.clone(),
    ));

    artifacts.final_reply = if needs_salvage && has_workspace_evidence {
        artifacts.events.push(AgentEvent::emit_kind(
            AgentId::Yazg,
            crate::types::AgentEventKind::React,
            "stage=salvage mode=workspace_evidence",
            conversation_id.clone(),
        ));
        synthesize_from_evidence(
            request.llms.supervisor.as_ref(),
            request.workspace_tools.as_deref(),
            &request.goal,
            artifacts.workspace_inventory.as_ref(),
            &workspace_obs,
            last_workspace_obs.as_deref(),
            if raw_workspace_dump {
                SalvageMode::PolishStructured
            } else {
                SalvageMode::JudgeRelevance
            },
        )
        .await
    } else if needs_salvage {
        if artifacts.summary.is_some()
            || artifacts.plan.is_some()
            || artifacts.analyze.is_some()
            || artifacts.recommend.is_some()
            || artifacts.generate_prompt.is_some()
            || artifacts.judge.is_some()
            || artifacts.created_project.is_some()
            || artifacts.pending_action.is_some()
        {
            artifacts.events.push(AgentEvent::emit_kind(
                AgentId::Yazg,
                crate::types::AgentEventKind::React,
                "stage=salvage mode=specialist_placeholder",
                conversation_id.clone(),
            ));
            "Specialist finished; synthesizing UI reply.".into()
        } else {
            artifacts.events.push(AgentEvent::emit_kind(
                AgentId::Yazg,
                crate::types::AgentEventKind::React,
                "stage=salvage mode=text_reply",
                conversation_id.clone(),
            ));
            recover_text_reply(request.llms.supervisor.as_ref(), &request.goal).await
        }
    } else {
        artifacts.events.push(AgentEvent::emit_kind(
            AgentId::Yazg,
            crate::types::AgentEventKind::React,
            format!(
                "stage=direct_reply chars={}",
                normalized_reply.chars().count()
            ),
            conversation_id.clone(),
        ));
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
            content_json: Some(serde_json::json!({
                "event": "assistant_message",
                "session_id": request.memory_ctx.session_id,
                "tools_used": tools_used,
            })),
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

    extract_session_insights_to_ltm(
        request.memory.as_deref(),
        &request.memory_ctx,
        AgentId::Yazg,
        &request.goal,
        &artifacts.final_reply,
        &tools_used,
    )
    .await;

    artifacts.events.push(AgentEvent::emit_kind(
        AgentId::Yazg,
        crate::types::AgentEventKind::Completed,
        "Yazg agent loop finished",
        conversation_id.clone(),
    ));
    let tool_used = if tools_used.is_empty() {
        "None".to_string()
    } else {
        tools_used.join(",")
    };
    info!(
        capability = %loaded.capability,
        loaded_tools = loaded.tool_count(),
        tool_used = %tool_used,
        "yazg capability turn complete"
    );
    artifacts.events.push(AgentEvent::emit_kind(
        AgentId::Yazg,
        crate::types::AgentEventKind::Info,
        format!(
            "capability={} loaded_tools={} tool_used={}",
            loaded.capability,
            loaded.tool_count(),
            tool_used
        ),
        conversation_id.clone(),
    ));
    artifacts.events.push(AgentEvent::emit_kind(
        AgentId::Yazg,
        crate::types::AgentEventKind::Info,
        usage_note,
        conversation_id,
    ));

    soft_end_trace(active_trace.as_ref(), TraceStatus::Ok).await;

    Ok((artifacts, raw_output))
}

/// Capability-scoped runtime context (do not inject unrelated state).
fn capability_context_block(
    capability: AssistantCapability,
    specialist: &YazgSpecialistContext,
) -> String {
    match capability {
        AssistantCapability::Conversation | AssistantCapability::Knowledge => String::new(),
        AssistantCapability::Targets | AssistantCapability::Attack | AssistantCapability::Scan => {
            specialist.format_context_block()
        }
        AssistantCapability::Projects
        | AssistantCapability::Workspace
        | AssistantCapability::Findings
        | AssistantCapability::Reports => {
            // Inventory capabilities do not need bound-target / factory flags.
            String::new()
        }
        AssistantCapability::Models | AssistantCapability::Runtime | AssistantCapability::Settings => {
            String::new()
        }
    }
}

/// Build only the Rig tools owned by the resolved capability (never the global set).
fn build_capability_tools(
    loaded: &LoadedCapabilityTools,
    request: &YazgRequest,
    state: &SharedYazgState,
    specialist: Arc<YazgSpecialistContext>,
) -> Vec<Box<dyn ToolDyn>> {
    if !loaded.allow_tool_calling || loaded.tool_names.is_empty() {
        return Vec::new();
    }
    let allowed: std::collections::HashSet<&str> =
        loaded.tool_names.iter().copied().collect();
    let mut out: Vec<Box<dyn ToolDyn>> = Vec::new();

    if allowed.contains("analyze_endpoint") {
        out.push(Box::new(AnalyzeEndpointTool {
            ctx: specialist.clone(),
            llm: request.llms.analyze.clone(),
            state: state.clone(),
        }));
    }
    if let Some(tools) = request.workspace_tools.clone() {
        if allowed.contains("list_workspace") {
            out.push(Box::new(ListWorkspaceTool {
                tools: tools.clone(),
                state: state.clone(),
            }));
        }
        if allowed.contains("project_detail") {
            out.push(Box::new(ProjectDetailTool {
                tools: tools.clone(),
                state: state.clone(),
            }));
        }
        if allowed.contains("list_scan") {
            out.push(Box::new(ListScanTool {
                tools: tools.clone(),
                state: state.clone(),
            }));
        }
        if allowed.contains("scan_detail") {
            out.push(Box::new(ScanDetailTool {
                tools: tools.clone(),
                state: state.clone(),
            }));
        }
        if allowed.contains("list_findings") {
            out.push(Box::new(ListFindingsTool {
                tools: tools.clone(),
                state: state.clone(),
            }));
        }
        if allowed.contains("finding_detail") {
            out.push(Box::new(FindingDetailTool {
                tools: tools.clone(),
                state: state.clone(),
            }));
        }
        if allowed.contains("list_targets") {
            out.push(Box::new(ListTargetsTool {
                tools: tools.clone(),
                state: state.clone(),
            }));
        }
        if allowed.contains("target_detail") {
            out.push(Box::new(TargetDetailTool {
                tools: tools.clone(),
                state: state.clone(),
            }));
        }
        if allowed.contains("list_reports") {
            out.push(Box::new(ListReportsTool {
                tools: tools.clone(),
                state: state.clone(),
            }));
        }
        if allowed.contains("report_detail") {
            out.push(Box::new(ReportDetailTool {
                tools: tools.clone(),
                state: state.clone(),
            }));
        }
    }
    if allowed.contains("create_project") {
        if request.project_tools.is_some() {
            if let Some(tools) = request.workspace_tools.clone() {
                out.push(Box::new(CreateProjectTool {
                    tools,
                    state: state.clone(),
                }));
            }
        }
    }
    if allowed.contains("attack_plan") {
        out.push(Box::new(AttackPlanTool {
            ctx: specialist.clone(),
            llm: request.llms.plan.clone(),
            state: state.clone(),
        }));
    }
    if allowed.contains("generate_prompt") {
        out.push(Box::new(GeneratePromptTool {
            ctx: specialist.clone(),
            llm: request.llms.prompt.clone(),
            state: state.clone(),
        }));
    }
    if allowed.contains("recommend") {
        out.push(Box::new(RecommendTool {
            ctx: specialist.clone(),
            llm: request.llms.recommend.clone(),
            state: state.clone(),
        }));
    }
    if allowed.contains("summary") {
        out.push(Box::new(SummaryTool {
            ctx: specialist.clone(),
            llm: request.llms.summary.clone(),
            state: state.clone(),
        }));
    }
    if allowed.contains("judge") {
        out.push(Box::new(JudgeTool {
            ctx: specialist,
            orchestrator: request.llms.judge.clone(),
            state: state.clone(),
        }));
    }
    out
}

/// AgentTrace span for capability routing — name auto-filled as `capability_classify`.
async fn capability_classify(
    trace: Option<&TraceHandle>,
    inputs: serde_json::Value,
    model_label: Option<&str>,
) -> Option<promptlab_agenttrace::SpanHandle> {
    let mut attributes = std::collections::BTreeMap::new();
    if let Some(model) = model_label {
        attributes.insert("model".into(), model.to_string());
    }
    promptlab_agenttrace::start_span!(
        trace,
        SpanStart {
            name: String::new(),
            kind: SpanKind::Capability,
            parent_span_id: None,
            inputs: Some(inputs),
            attributes,
        }
    )
    .await
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

/// Map session STM events → prior Rig chat messages (user/assistant only).
///
/// Skips the current user turn (already passed as `prompt`) and tool/observation
/// rows (final assistant reply already closes those turns for chat continuity).
async fn load_prior_chat_history(
    memory: Option<&dyn AgentMemoryStore>,
    session_id: &str,
    current_user: &str,
) -> Vec<Message> {
    let Some(store) = memory else {
        return Vec::new();
    };
    if session_id.trim().is_empty() {
        return Vec::new();
    }
    let Ok(mut entries) = store
        .stm_list(session_id, Some(AgentId::Yazg.as_str()), 48)
        .await
    else {
        return Vec::new();
    };

    // Drop the STM user event we just appended for this turn.
    if let Some(last) = entries.last() {
        if last.role == "user" && last.content.trim() == current_user.trim() {
            entries.pop();
        }
    }

    stm_entries_to_chat_messages(&entries)
}

fn stm_entries_to_chat_messages(entries: &[StmEntry]) -> Vec<Message> {
    let mut messages = Vec::new();
    for entry in entries {
        let content = entry.content.trim();
        if content.is_empty() {
            continue;
        }
        match entry.role.as_str() {
            "user" => {
                messages.push(Message::User {
                    content: OneOrMany::one(UserContent::text(content)),
                });
            }
            "assistant" => {
                messages.push(Message::Assistant {
                    id: None,
                    content: OneOrMany::one(AssistantContent::text(content)),
                });
            }
            // tool / observation / system: kept in STM for Agent Trace, not replayed
            // as incomplete OpenAI tool turns on the next chat prompt.
            _ => {}
        }
    }
    messages
}

/// Pick the Observation that best matches the user goal (not merely the last one).
pub(crate) fn pick_best_workspace_observation<'a>(
    goal: &str,
    observations: &'a [String],
    last_obs: Option<&'a str>,
) -> Option<&'a str> {
    let g = goal.to_lowercase();
    let prefer_tools: &[&str] = if g.contains("target") {
        &["list_targets", "target_detail", "project_detail"]
    } else if g.contains("finding") || g.contains("lỗ hổng") || g.contains("lo hong") {
        &["finding_detail", "list_findings", "scan_detail"]
    } else if g.contains("scan") {
        &["list_scan", "scan_detail", "project_detail"]
    } else if g.contains("report") {
        &["list_reports", "report_detail"]
    } else if g.contains("project") || g.contains("information") || g.contains("info") {
        &["project_detail", "list_workspace"]
    } else {
        &[]
    };

    for tool in prefer_tools {
        if let Some(obs) = observations
            .iter()
            .rev()
            .find(|o| obs_tool_name(o).as_deref() == Some(*tool))
            .map(|s| s.as_str())
        {
            return Some(obs);
        }
    }
    if let Some(obs) = observations
        .iter()
        .rev()
        .find(|o| ToolResult::parse(o).is_some_and(|r| r.is_error()))
        .map(|s| s.as_str())
    {
        return Some(obs);
    }
    observations
        .last()
        .map(|s| s.as_str())
        .or(last_obs)
}

fn obs_tool_name(obs: &str) -> Option<String> {
    ToolResult::parse(obs).map(|r| r.tool)
}

/// Deterministic Finish: JSON ToolResult → user markdown; legacy prose fallback.
pub(crate) fn observation_to_user_markdown(obs: &str) -> String {
    if let Some(tr) = ToolResult::parse(obs) {
        if let Some(md) = tr.error_user_markdown() {
            return md;
        }
        if tr.is_ok() {
            if let Some(data) = tr.data.clone() {
                match tr.tool.as_str() {
                    "list_workspace" => {
                        if let Ok(inv) = serde_json::from_value::<WorkspaceInventory>(data) {
                            return inv.compact_user_reply_for_goal("");
                        }
                    }
                    "list_targets" => {
                        if let Ok(list) = serde_json::from_value::<TargetList>(data) {
                            return format_target_list_markdown(&list);
                        }
                    }
                    "project_detail"
                    | "scan_detail"
                    | "finding_detail"
                    | "target_detail"
                    | "list_scan"
                    | "list_findings"
                    | "list_reports"
                    | "report_detail"
                    | "create_project" => {
                        return format_ok_data_markdown(&tr.tool, &data);
                    }
                    _ => {}
                }
            }
        }
    }
    // Non-JSON fallback: strip routing lines only.
    let body: Vec<&str> = obs
        .lines()
        .filter(|line| {
            let t = line.trim();
            !t.is_empty()
                && !t.starts_with("Next:")
                && !t.starts_with("Reply:")
        })
        .collect();
    body.join("\n")
}

fn format_target_list_markdown(list: &TargetList) -> String {
    let mut lines = vec![format!("### Targets in {}", list.project_name)];
    if list.targets.is_empty() {
        lines.push("- (none)".into());
    } else {
        for t in &list.targets {
            lines.push(format!(
                "- **{}** (`{}`) — {}",
                t.name, t.id, t.target_type
            ));
        }
    }
    lines.join("\n")
}

fn format_ok_data_markdown(tool: &str, data: &serde_json::Value) -> String {
    match data {
        serde_json::Value::Object(map) => {
            let mut lines = vec![format!("### {tool}")];
            for (k, v) in map {
                if k.ends_with("_json") || k == "evidenceJson" || k == "evidence_json" {
                    continue;
                }
                if let serde_json::Value::Array(arr) = v {
                    lines.push(format!("**{k}** ({})", arr.len()));
                    for item in arr.iter().take(12) {
                        if let Some(b) = candidate_line(item) {
                            lines.push(b);
                        }
                    }
                    if arr.len() > 12 {
                        lines.push(format!("- … +{} more", arr.len() - 12));
                    }
                } else if let serde_json::Value::Object(obj) = v {
                    let name = obj
                        .get("name")
                        .or_else(|| obj.get("title"))
                        .and_then(|x| x.as_str());
                    let id = obj.get("id").and_then(|x| x.as_str());
                    match (name, id) {
                        (Some(n), Some(i)) => lines.push(format!("- **{n}** (`{i}`)")),
                        (Some(n), None) => lines.push(format!("- **{n}**")),
                        _ => lines.push(format!("- {k}: {v}")),
                    }
                } else if !v.is_null() {
                    lines.push(format!("- **{k}**: {v}"));
                }
            }
            lines.join("\n")
        }
        _ => data.to_string(),
    }
}

fn candidate_line(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::Object(map) => {
            let name = map
                .get("name")
                .or_else(|| map.get("title"))
                .and_then(|x| x.as_str());
            let id = map.get("id").and_then(|x| x.as_str());
            match (name, id) {
                (Some(n), Some(i)) => Some(format!("- **{n}** (`{i}`)")),
                (Some(n), None) => Some(format!("- **{n}**")),
                (None, Some(i)) => Some(format!("- `{i}`")),
                _ => None,
            }
        }
        serde_json::Value::String(s) => Some(format!("- {s}")),
        _ => None,
    }
}

async fn recover_text_reply(llm: &dyn PlannerLlm, goal: &str) -> String {
    let prompt = format!(
        "User message:\n{goal}\n\n\
         Reply helpfully as Yazg in natural language. Be concise. \
         Do not call tools. Do not invent workspace rows."
    );
    match llm
        .complete_with_system(
            Some(
                "You are Yazg, PromptLab's AI assistant for authorized AI security testing. \
                 Answer the user directly.",
            ),
            &prompt,
        )
        .await
    {
        Ok(text) => {
            let normalized = crate::yazg_model::normalize_user_facing_reply(&text);
            if normalized.trim().is_empty()
                || crate::yazg_model::reply_looks_like_agent_meta(&normalized)
            {
                crate::yazg_model::EMPTY_FALLBACK_REPLY.into()
            } else {
                normalized
            }
        }
        Err(err) => {
            warn!(error = %err, "text reply recovery failed");
            crate::yazg_model::EMPTY_FALLBACK_REPLY.into()
        }
    }
}

/// Finish salvage modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SalvageMode {
    /// Empty / meta reply — model must decide if obs is relevant.
    JudgeRelevance,
    /// Model already answered from obs but leaked raw tool text — format only.
    PolishStructured,
}

/// Finish salvage: answer from ToolResult JSON (generic), or recover text.
async fn synthesize_from_evidence(
    llm: &dyn PlannerLlm,
    workspace_tools: Option<&dyn WorkspaceTools>,
    goal: &str,
    inventory: Option<&WorkspaceInventory>,
    observations: &[String],
    last_obs: Option<&str>,
    mode: SalvageMode,
) -> String {
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
    let parsed_best = best.and_then(ToolResult::parse);
    let deterministic = best
        .map(observation_to_user_markdown)
        .filter(|s| !s.trim().is_empty());

    // Skipped (e.g. project_detail("yazg")) → normal chat recovery.
    if parsed_best.as_ref().is_some_and(|r| r.is_skipped()) {
        return recover_text_reply(llm, goal).await;
    }

    // Deterministic error markdown for not_found / validation / empty.
    if let Some(tr) = parsed_best.as_ref() {
        if tr.is_error() {
            if mode == SalvageMode::PolishStructured {
                if let Some(md) = tr.error_user_markdown() {
                    return md;
                }
            }
            // JudgeRelevance: still prefer structured miss text when we have candidates;
            // LLM path below handles "irrelevant → ignore".
            if tr.is_not_found() {
                if let Some(md) = tr.error_user_markdown() {
                    // Defer to LLM only when there is no actionable message.
                    if !md.trim().is_empty() && mode == SalvageMode::PolishStructured {
                        return md;
                    }
                }
            }
        }
    }

    if mode == SalvageMode::PolishStructured {
        if let Some(tr) = parsed_best.as_ref() {
            if tr.is_ok() && tr.tool == "list_workspace" {
                if let Some(inv) = inventory {
                    return inv.compact_user_reply_for_goal(goal);
                }
            }
            if tr.is_ok() {
                if let Some(md) = deterministic {
                    return md;
                }
            }
            if tr.is_error() {
                if let Some(md) = tr.error_user_markdown() {
                    return md;
                }
            }
        }
        if let Some(md) = deterministic {
            return md;
        }
        if let Some(inv) = inventory {
            return inv.compact_user_reply_for_goal(goal);
        }
    }

    let observation = best
        .map(str::to_string)
        .or_else(|| last_obs.map(str::to_string))
        .or_else(|| {
            inventory.map(|i| ToolResult::ok("list_workspace", i).to_json_string())
        })
        .unwrap_or_default();

    let prompt = if observation.trim().is_empty() {
        format!(
            "User question:\n{goal}\n\n\
             No usable tool result. Answer the user in natural language. Be concise. \
             Do not invent workspace projects/targets."
        )
    } else {
        format!(
            "User question:\n{goal}\n\nTool result JSON:\n{observation}\n\n\
             CLOSED DOMAIN — answer ONLY from entities present in the tool JSON.\n\
             Privately decide relevance, then output ONLY the final user-visible reply.\n\
             - status=ok → short natural markdown with names/ids from data.\n\
             - status=error → use message and list candidates[] when present.\n\
             - Irrelevant (greeting/chat) or error_class=skipped → ignore JSON; answer naturally.\n\
             Never invent entities. Never rename another entity to match the user's requested name.\n\
             Forbidden: Yes/No narration, tool names, raw JSON dumps, ``` fences around the whole reply."
        )
    };

    match llm
        .complete_with_system(
            Some(
                "You are Yazg. Closed-domain workspace assistant. \
                 Output only the final answer. Never invent entities. Never narrate decisions.",
            ),
            &prompt,
        )
        .await
    {
        Ok(text) => {
            let normalized = crate::yazg_model::normalize_user_facing_reply(&text);
            if normalized.trim().is_empty()
                || crate::yazg_model::reply_looks_like_agent_meta(&normalized)
                || looks_like_raw_workspace_obs(&normalized)
            {
                if let Some(tr) = parsed_best.as_ref() {
                    if tr.is_error() {
                        if let Some(md) = tr.error_user_markdown() {
                            return md;
                        }
                    }
                }
                if looks_like_raw_workspace_obs(&normalized) {
                    if let Some(tr) = parsed_best.as_ref() {
                        if tr.tool == "list_workspace" {
                            if let Some(inv) = inventory {
                                return inv.compact_user_reply_for_goal(goal);
                            }
                        }
                    }
                    if let Some(md) = deterministic {
                        return md;
                    }
                }
                recover_text_reply(llm, goal).await
            } else {
                // Guard remapping: not_found but reply invents another inventory project name.
                if let Some(tr) = parsed_best.as_ref() {
                    if tr.is_not_found() {
                        if let (Some(md), Some(inv)) = (tr.error_user_markdown(), inventory) {
                            let invents_other = inv.projects.iter().any(|p| {
                                normalized.to_lowercase().contains(&p.name.to_lowercase())
                            });
                            if invents_other {
                                return md;
                            }
                        }
                    }
                }
                normalized
            }
        }
        Err(err) => {
            warn!(error = %err, "evidence reply synthesis failed");
            if let Some(tr) = parsed_best.as_ref() {
                if let Some(md) = tr.error_user_markdown() {
                    return md;
                }
            }
            recover_text_reply(llm, goal).await
        }
    }
}

fn looks_like_raw_workspace_obs(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.contains("\"status\"")
        && (lower.contains("\"tool\"") || lower.contains("error_class"))
        || lower.contains("list_workspace ok")
        || lower.contains("list_targets ok")
        || lower.contains("project_detail ok")
        || lower.contains(" not_found")
        || (lower.contains("id=") && lower.contains("name=") && lower.contains("type="))
        || (lower.contains("id=")
            && lower.contains("name=")
            && (lower.contains("targets=") || lower.contains("findings=")))
}

fn extract_project_hint(goal: &str, inventory: Option<&WorkspaceInventory>) -> Option<String> {
    if let Some(asked) = extract_requested_project_name(goal) {
        if let Some(inv) = inventory {
            if let Some(p) = inv.find_project(&asked) {
                return Some(p.name.clone());
            }
        }
        return Some(asked);
    }
    if let Some(inv) = inventory {
        return inv.project_named_in_goal(goal).map(|p| p.name.clone());
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
    use super::{
        extract_requested_project_name, observation_to_user_markdown,
        pick_best_workspace_observation,
    };
    use crate::tool_result::ToolResult;
    use serde_json::json;

    fn ok_obs(tool: &str, data: serde_json::Value) -> String {
        ToolResult::ok(tool, data).to_json_string()
    }

    fn not_found_obs(tool: &str, message: &str, candidates: Vec<serde_json::Value>) -> String {
        ToolResult::not_found(
            tool,
            message,
            candidates,
            vec!["List candidates; do not invent".into()],
        )
        .to_json_string()
    }

    #[test]
    fn pick_targets_obs_over_list_workspace() {
        let obs = vec![
            ok_obs("list_workspace", json!({"projects": [], "totals": {}})),
            ok_obs(
                "list_targets",
                json!({
                    "projectId": "p1",
                    "projectName": "AI",
                    "targets": [{"id": "t1", "projectId": "p1", "name": "10.0.0.1", "targetType": "llm_api"}]
                }),
            ),
        ];
        let best = pick_best_workspace_observation(
            "Give me all target in project AI",
            &obs,
            None,
        );
        assert_eq!(ToolResult::parse(best.unwrap()).unwrap().tool, "list_targets");
    }

    #[test]
    fn pick_project_detail_for_info_goal() {
        let obs = vec![
            ok_obs("list_workspace", json!({"projects": []})),
            ok_obs("project_detail", json!({"project": {"id": "p1", "name": "AI"}})),
            ok_obs("list_findings", json!({"findings": []})),
        ];
        let best =
            pick_best_workspace_observation("give me information of project AI", &obs, None);
        assert_eq!(ToolResult::parse(best.unwrap()).unwrap().tool, "project_detail");
    }

    #[test]
    fn pick_not_found_for_named_project_goal() {
        let obs = vec![
            ok_obs("list_workspace", json!({"projects": []})),
            not_found_obs(
                "project_detail",
                "No project matching `WebApp`",
                vec![json!({"id": "p1", "name": "AI"})],
            ),
        ];
        let best = pick_best_workspace_observation(
            "cho tôi thông tin project WebApp",
            &obs,
            None,
        );
        let tr = ToolResult::parse(best.unwrap()).unwrap();
        assert!(tr.is_not_found());
        assert_eq!(tr.tool, "project_detail");
    }

    #[test]
    fn pick_not_found_for_missing_target_goal() {
        let obs = vec![
            ok_obs("list_workspace", json!({"projects": []})),
            not_found_obs(
                "target_detail",
                "No target matching `missing`",
                vec![json!({"id": "t1", "name": "api"})],
            ),
        ];
        // Goal mentions target so prefer-list applies; only target_detail miss is present.
        let best = pick_best_workspace_observation(
            "target detail for missing in project AI",
            &obs,
            None,
        );
        assert_eq!(ToolResult::parse(best.unwrap()).unwrap().tool, "target_detail");
    }

    #[test]
    fn greeting_falls_back_to_error_obs() {
        let obs = vec![ToolResult::skipped(
            "project_detail",
            "`yazg` is the assistant name, not a workspace project",
            vec![],
        )
        .to_json_string()];
        let best = pick_best_workspace_observation("hello", &obs, None);
        assert!(ToolResult::parse(best.unwrap()).unwrap().is_skipped());
        assert_eq!(extract_requested_project_name("hello"), None);
    }

    #[test]
    fn formats_json_list_targets_for_user() {
        let md = observation_to_user_markdown(&ok_obs(
            "list_targets",
            json!({
                "projectId": "p1",
                "projectName": "AI",
                "targets": [
                    {"id": "t1", "projectId": "p1", "name": "10.0.0.1", "targetType": "llm_api"}
                ]
            }),
        ));
        assert!(md.contains("10.0.0.1"));
        assert!(md.contains("### Targets in AI"));
        assert!(!md.contains("\"status\""));
    }

    #[test]
    fn formats_json_list_workspace_for_user() {
        let md = observation_to_user_markdown(&ok_obs(
            "list_workspace",
            json!({
                "projects": [{
                    "id": "p1",
                    "name": "AI",
                    "targetsCount": 2,
                    "scansCount": 2,
                    "findingsCount": 36
                }],
                "totals": {
                    "projects": 1,
                    "targets": 2,
                    "scans": 2,
                    "findings": 36
                }
            }),
        ));
        assert!(md.contains("**AI**"));
        assert!(md.contains("`p1`"));
    }

    #[test]
    fn formats_json_not_found_for_user() {
        let md = observation_to_user_markdown(&not_found_obs(
            "project_detail",
            "No project matching `WebApp`",
            vec![json!({"id": "p1", "name": "AI"})],
        ));
        assert!(md.contains("WebApp"));
        assert!(md.contains("AI"));
        assert!(!md.contains("error_class"));
    }

    #[test]
    fn formats_target_not_found_json() {
        let md = observation_to_user_markdown(&not_found_obs(
            "target_detail",
            "No target matching `api-prod`",
            vec![
                json!({"id": "t1", "name": "api-dev"}),
                json!({"id": "t2", "name": "api-stg"}),
            ],
        ));
        assert!(md.contains("api-prod"));
        assert!(md.contains("api-dev"));
    }

    #[test]
    fn extracts_requested_project_name() {
        assert_eq!(
            extract_requested_project_name("cho tôi thông tin project WebApp").as_deref(),
            Some("WebApp")
        );
        assert_eq!(
            extract_requested_project_name("give me information of project AI").as_deref(),
            Some("AI")
        );
        assert_eq!(
            extract_requested_project_name("workspace có project nào"),
            None
        );
        assert_eq!(
            extract_requested_project_name("cho tôi project có trong workspace"),
            None
        );
    }

    #[test]
    fn stm_history_is_user_assistant_messages_not_tools() {
        use super::stm_entries_to_chat_messages;
        use crate::memory::StmEntry;
        use rig::completion::Message;

        let entries = vec![
            StmEntry {
                id: "1".into(),
                agent_id: "yazg".into(),
                role: "user".into(),
                memory_key: Some("message".into()),
                content: "hi".into(),
                content_json: None,
                importance: 0.7,
                created_at: None,
            },
            StmEntry {
                id: "2".into(),
                agent_id: "yazg".into(),
                role: "assistant".into(),
                memory_key: Some("reply".into()),
                content: "Hello!".into(),
                content_json: None,
                importance: 0.6,
                created_at: None,
            },
            StmEntry {
                id: "3".into(),
                agent_id: "yazg".into(),
                role: "tool".into(),
                memory_key: Some("list_workspace".into()),
                content: "tool_call:list_workspace".into(),
                content_json: None,
                importance: 0.5,
                created_at: None,
            },
        ];
        let msgs = stm_entries_to_chat_messages(&entries);
        assert_eq!(msgs.len(), 2);
        assert!(matches!(msgs[0], Message::User { .. }));
        assert!(matches!(msgs[1], Message::Assistant { .. }));
    }
}
