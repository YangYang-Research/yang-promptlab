//! Yazg multi-agent orchestration.
//!
//! Hierarchy (v1):
//! ```text
//! Yazg (Supervisor)
//! ├── AnalyzeEndpointAgent  — HTTP capability probe + Yazg AI classification
//! ├── AttackPlanAgent       — wizard attack plan from verified profile
//! └── GeneratePromptAgent   — Attack Factory novel technique probe
//! ```

pub mod analyze_endpoint;
pub mod attack_plan;
pub mod error;
pub mod generate_prompt;
pub mod react;
pub mod supervisor;
pub mod types;

pub use analyze_endpoint::{AnalyzeEndpointAgent, AnalyzeEndpointAgentOutcome};
pub use attack_plan::{AttackPlanAgent, AttackPlanAgentOutcome};
pub use error::{AgentError, AgentResult};
pub use generate_prompt::{
    GeneratePromptAgent, GeneratePromptAgentOutcome, TechniquePromptContext,
};
pub use react::{ReactActionKind, ReactArtifacts, ReactRequest};
pub use supervisor::{SupervisorIntent, YazgDelegation, YazgSupervisor, YazgTurn};
pub use types::{AgentEvent, AgentEventKind, AgentId};
