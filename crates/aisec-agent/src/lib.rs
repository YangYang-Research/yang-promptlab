//! Autonomous agentic attack scanner for AISec.

pub mod engine;
pub mod error;
pub mod host;
pub mod plan;
pub mod retry;
pub mod types;

pub use engine::{run_category_episode, run_endpoint_agent};
pub use error::{AgentError, AgentResult};
pub use host::AgentHost;
pub use plan::{intersect_categories, plan_attacks};
pub use retry::{generator_mode_for_retry, should_retry};
pub use types::{
    AgentConfig, AgentPhase, AgentScanResult, AgentStopReason, AgentVerdict,
    AttackExecutionSummary, CategoryAgentResult, PhaseRecord,
};
