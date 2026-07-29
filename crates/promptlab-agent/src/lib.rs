//! Yazg multi-agent orchestration (manager + domain tools).
//!
//! Hierarchy:
//! ```text
//! Yazg (Manager)
//! ├── list_workspace      — projects + counts only
//! ├── project_detail      — one project + targets + scans
//! ├── list_targets        — targets for a project
//! ├── target_detail       — one target + profile summary
//! ├── list_scan           — scans for a project
//! ├── scan_detail         — one scan + capped findings
//! ├── list_findings       — paginated findings
//! ├── finding_detail      — one finding
//! ├── list_reports        — reports for a project / all
//! ├── report_detail       — one report preview
//! ├── create_project      — tool → CreateProjectTools
//! ├── analyze_endpoint    — tool → AnalyzeEndpointAgent::run
//! ├── attack_plan         — tool → AttackPlanAgent::run
//! ├── generate_prompt     — tool → GeneratePromptAgent::run
//! ├── recommend           — tool → RecommendAgent::run
//! ├── summary             — tool → SummaryAgent::run
//! ├── judge               — tool → JudgeCoordinator (+ role vote tools)
//! ├── AgenticAttackExecutionAgent    — pick + host execute bridge
//! ├── SequentialAttackExecutionAgent — pick + host execute bridge
//! └── ReflectionAgent                — structured extractor
//! ```

pub mod agent_log;
pub mod analyze_endpoint;
pub mod artifacts;
pub mod attack_execution;
pub mod attack_execution_pick;
pub mod attack_plan;
pub mod create_project;
pub mod endpoint_recovery;
pub mod error;
pub mod generate_prompt;
pub mod judge_coordinator;
pub mod judge_workers;
pub mod list_workspace;
pub mod memory;
pub mod recommend;
pub mod reflection;
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
pub use judge_coordinator::{JudgeCoordinatorAgent, JudgeCoordinatorAgentOutcome};
pub use judge_workers::{
    AttackerWorker, ClassifierWorker, JudgeWorker, JudgeWorkerOutcome,
};
pub use list_workspace::{
    clamp_findings_limit, parse_finding_index, FindingDetail, FindingList, ProjectDetail,
    ReportDetail, ReportList, ScanDetail, ScanList, TargetDetail, TargetList,
    WorkspaceFindingSummary, WorkspaceInventory, WorkspaceProjectSummary, WorkspaceReportSummary,
    WorkspaceScanSummary, WorkspaceTargetSummary, WorkspaceTools, WorkspaceTotals,
    DEFAULT_FINDINGS_LIMIT, MAX_FINDINGS_LIMIT, MAX_REPORT_PREVIEW_CHARS,
};
pub use memory::{
    AgentMemoryStore, LtmEntry, LtmWrite, MemoryContext, MemoryScopeType, StmEntry, StmRole,
    StmWrite,
};
pub use recommend::{RecommendAgent, RecommendAgentOutcome};
pub use reflection::{ReflectionAgent, ReflectionOutcome, ReflectionRequest};
pub use yazg_runtime::{run_yazg, YazgRequest};
pub use yazg_tools::{YazgLlms, YazgSpecialistContext};
pub use sequential_attack_execution::{
    SequentialAttackExecutionAgent, SequentialAttackExecutionRequest,
};
pub use summary::{SummaryAgent, SummaryAgentOutcome, SummaryRequest};
pub use supervisor::{SupervisorIntent, YazgDelegation, YazgSupervisor, YazgTurn};
pub use types::{AgentEvent, AgentEventKind, AgentId};
