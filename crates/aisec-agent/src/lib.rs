//! Yazg multi-agent orchestration.
//!
//! Hierarchy:
//! ```text
//! Yazg (Supervisor)
//! ├── AnalyzeEndpointAgent  — HTTP capability probe + Yazg AI classification
//! ├── AttackPlanAgent       — wizard attack plan from verified profile
//! ├── GeneratePromptAgent   — Attack Factory novel technique probe
//! ├── RecommendAgent        — post-scan remediation recommendations
//! ├── SummaryAgent          — project / scan posture summaries
//! └── JudgeCoordinatorAgent — consensus judging via role workers
//!     ├── JudgeWorker
//!     ├── ClassifierWorker
//!     └── AttackerWorker
//! ```

pub mod analyze_endpoint;
pub mod attack_plan;
pub mod error;
pub mod generate_prompt;
pub mod judge_coordinator;
pub mod judge_workers;
pub mod recommend;
pub mod react;
pub mod summary;
pub mod supervisor;
pub mod types;

pub use analyze_endpoint::{AnalyzeEndpointAgent, AnalyzeEndpointAgentOutcome};
pub use attack_plan::{AttackPlanAgent, AttackPlanAgentOutcome};
pub use error::{AgentError, AgentResult};
pub use generate_prompt::{
    GeneratePromptAgent, GeneratePromptAgentOutcome, TechniquePromptContext,
};
pub use judge_coordinator::{JudgeCoordinatorAgent, JudgeCoordinatorAgentOutcome};
pub use judge_workers::{
    AttackerWorker, ClassifierWorker, JudgeWorker, JudgeWorkerOutcome,
};
pub use recommend::{RecommendAgent, RecommendAgentOutcome};
pub use react::{ReactActionKind, ReactArtifacts, ReactLlms, ReactRequest};
pub use summary::{SummaryAgent, SummaryAgentOutcome, SummaryRequest};
pub use supervisor::{SupervisorIntent, YazgDelegation, YazgSupervisor, YazgTurn};
pub use types::{AgentEvent, AgentEventKind, AgentId};
