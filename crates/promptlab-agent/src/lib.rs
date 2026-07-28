//! Yazg multi-agent orchestration.
//!
//! Hierarchy:
//! ```text
//! Yazg (Supervisor — Rig Agent)
//! ├── AnalyzeEndpointAgent  — HTTP capability probe + Yazg AI classification
//! ├── AttackPlanAgent       — wizard attack plan + mid-scan adapt
//! ├── GeneratePromptAgent   — Attack Factory novel technique probe
//! ├── RecommendAgent        — post-scan remediation recommendations
//! ├── SummaryAgent          — project / scan posture summaries
//! ├── CreateProjectTool     — create a workspace project (host SQLite)
//! ├── ListWorkspaceTool     — list projects/targets/scans/findings from DB
//! ├── JudgeCoordinatorAgent — consensus judging via role workers
//! │   ├── JudgeWorker
//! │   ├── ClassifierWorker
//! │   └── AttackerWorker
//! ├── AgenticAttackExecutionAgent    — Rig AgentBuilder pick + host execute
//! ├── SequentialAttackExecutionAgent — Rig AgentBuilder pick + host execute
//! └── ReflectionAgent       — Rig Extractor (structured submit)
//! ```

pub mod agent_log;
pub mod analyze_endpoint;
pub mod artifacts;
pub mod attack_execution;
pub mod attack_execution_rig;
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
pub mod rig_model;
pub mod rig_runtime;
pub mod rig_tools;
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
    WorkspaceFindingSummary, WorkspaceInventory, WorkspaceProjectSummary, WorkspaceScanSummary,
    WorkspaceTargetSummary, WorkspaceTools, WorkspaceTotals,
};
pub use memory::{
    AgentMemoryStore, LtmEntry, LtmWrite, MemoryContext, MemoryScopeType, StmEntry, StmRole,
    StmWrite,
};
pub use recommend::{RecommendAgent, RecommendAgentOutcome};
pub use reflection::{ReflectionAgent, ReflectionOutcome, ReflectionRequest};
pub use rig_runtime::{run_yazg_rig, YazgRigRequest};
pub use rig_tools::{YazgRigLlms, YazgSpecialistContext};
pub use sequential_attack_execution::{
    SequentialAttackExecutionAgent, SequentialAttackExecutionRequest,
};
pub use summary::{SummaryAgent, SummaryAgentOutcome, SummaryRequest};
pub use supervisor::{SupervisorIntent, YazgDelegation, YazgSupervisor, YazgTurn};
pub use types::{AgentEvent, AgentEventKind, AgentId};
