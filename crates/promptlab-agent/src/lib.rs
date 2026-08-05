//! Yazg multi-agent orchestration (manager + capability-routed domain tools).
//!
//! Architecture:
//! ```text
//! User → LLM IntentRouter (capability, no tools)
//!      → CapabilityToolLoader
//!      → AI Runtime (capability tools only) → optional tool → Response
//! ```
//!
//! The tool-calling LLM never receives the full tool registry — only tools for the
//! capability chosen by the classifier LLM.
//!
//! Hierarchy (tools owned by capabilities, not injected globally):
//! ```text
//! Yazg (Manager)
//! ├── Conversation / Knowledge — zero tools
//! ├── Workspace / Projects     — list_workspace, project_detail, create_project
//! ├── Targets                  — list_targets, target_detail, analyze_endpoint
//! ├── Scan / Findings/Reports  — list_scan, scan_detail, list_findings, …
//! ├── Attack specialists       — attack_plan, generate_prompt, recommend, summary, judge
//! ├── AgenticAttackExecutionAgent / SequentialAttackExecutionAgent / ReflectionAgent
//! ```

pub mod agent_log;
pub mod analyze_endpoint;
pub mod artifacts;
pub mod assistant;
pub mod attack_execution;
pub mod attack_execution_pick;
pub mod attack_plan;
pub mod create_project;
pub mod endpoint_recovery;
pub mod error;
pub mod generate_prompt;
pub mod hilt;
pub mod judge_coordinator;
pub mod judge_workers;
pub mod list_workspace;
pub mod memory;
pub mod recommend;
pub mod reflection;
pub mod tool_result;
pub mod yazg_model;
pub mod yazg_prompts;
pub mod yazg_runtime;
pub mod yazg_tools;
pub mod sequential_attack_execution;
pub mod summary;
pub mod supervisor;
pub mod types;

pub use agent_log::{log_agent_event, log_llm_call, log_react, log_tool_call, AgentLogContext};
pub use analyze_endpoint::{AnalyzeEndpointAgent, AnalyzeEndpointAgentOutcome};
pub use artifacts::{persist_artifacts_ltm, YazgActionKind, YazgArtifacts};
pub use assistant::{
    AssistantCapability, CapabilityRegistry, CapabilityToolLoader, IntentResolution, IntentRouter,
    LoadedCapabilityTools, RouteInput,
};
pub use attack_execution::{
    emit_and_record, AgenticAttackExecutionAgent, AttackAttemptObservation, AttackExecutionLlms,
    AttackExecutionOutcome, AttackExecutionRequest, AttackExecutionTools,
};
pub use endpoint_recovery::{
    heuristic_recovery, observation_needs_recovery, seed_pacing_from_prior_failure, EndpointPacing,
    RecoveryPlan,
    DEFAULT_ATTACK_CONCURRENCY, DEFAULT_TIMEOUT_MS, MAX_ENDPOINT_RECOVERIES,
};
pub use attack_plan::{
    AdaptPlanOutcome, AdaptPlanRequest, AttackPlanAgent, AttackPlanAgentOutcome,
};
pub use create_project::{CreateProjectTools, CreatedProject};
pub use error::{AgentError, AgentResult};
pub use generate_prompt::{
    GeneratePromptAgent, GeneratePromptAgentOutcome, TechniquePromptContext,
};
pub use hilt::{
    is_mutating_tool, mutation_kind_for_tool, HiltMutationKind, HiltPendingAction, HILT_TTL_SECS,
};
pub use judge_coordinator::{JudgeCoordinatorAgent, JudgeCoordinatorAgentOutcome};
pub use judge_workers::{
    AttackerWorker, ClassifierWorker, JudgeWorker, JudgeWorkerOutcome,
};
pub use list_workspace::{
    clamp_findings_limit, extract_requested_project_name, parse_finding_index, FindingDetail,
    FindingList, ProjectDetail, ReportDetail, ReportList, ScanDetail, ScanList, TargetDetail,
    TargetList, WorkspaceFindingSummary, WorkspaceInventory, WorkspaceProjectSummary,
    WorkspaceReportSummary, WorkspaceScanSummary, WorkspaceTargetSummary, WorkspaceTools,
    WorkspaceTotals, DEFAULT_FINDINGS_LIMIT, MAX_FINDINGS_LIMIT, MAX_REPORT_PREVIEW_CHARS,
};
pub use memory::{
    extract_session_insights_to_ltm, AgentMemoryStore, LtmEntry, LtmWrite, MemoryContext,
    MemoryScopeType, StmEntry, StmRole, StmSessionSummary, StmWrite, STM_CONTENT_MAX_CHARS,
};
pub use recommend::{RecommendAgent, RecommendAgentOutcome};
pub use reflection::{ReflectionAgent, ReflectionOutcome, ReflectionRequest};
pub use tool_result::{ToolErrorClass, ToolResult, ToolStatus};
pub use yazg_runtime::{run_yazg, YazgRequest};
pub use yazg_tools::{YazgLlms, YazgSpecialistContext};
pub use sequential_attack_execution::{
    SequentialAttackExecutionAgent, SequentialAttackExecutionRequest,
};
pub use summary::{SummaryAgent, SummaryAgentOutcome, SummaryRequest};
pub use supervisor::{SupervisorIntent, YazgDelegation, YazgSupervisor, YazgTurn};
pub use types::{AgentEvent, AgentEventKind, AgentId};
