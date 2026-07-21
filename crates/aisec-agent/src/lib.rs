//! Yazg multi-agent orchestration.
//!
//! Hierarchy (v1):
//! ```text
//! Yazg (Supervisor)
//! ├── AnalyzeEndpointAgent  — HTTP capability probe + Yazg AI classification
//! └── AttackPlanAgent       — wizard attack plan from verified profile
//! ```

pub mod analyze_endpoint;
pub mod attack_plan;
pub mod error;
pub mod react;
pub mod supervisor;
pub mod types;

pub use analyze_endpoint::{AnalyzeEndpointAgent, AnalyzeEndpointAgentOutcome};
pub use attack_plan::{AttackPlanAgent, AttackPlanAgentOutcome};
pub use error::{AgentError, AgentResult};
pub use react::{ReactActionKind, ReactArtifacts, ReactRequest};
pub use supervisor::{SupervisorIntent, YazgDelegation, YazgSupervisor, YazgTurn};
pub use types::{AgentEvent, AgentEventKind, AgentId};
