use aisec_attack::AttackCategory;
use aisec_generator::GeneratorMode;
use aisec_planner::PlannerMode;
use serde::{Deserialize, Serialize};

/// Agent loop phase (Fingerprint → Plan → Attack → Judge → Retry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentPhase {
    Fingerprint,
    Plan,
    Generate,
    Attack,
    Judge,
    Retry,
    Complete,
}

impl AgentPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fingerprint => "fingerprint",
            Self::Plan => "plan",
            Self::Generate => "generate",
            Self::Attack => "attack",
            Self::Judge => "judge",
            Self::Retry => "retry",
            Self::Complete => "complete",
        }
    }
}

/// Why an agent episode stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStopReason {
    VulnerabilityFound,
    MaxAttemptsReached,
    CategoryComplete,
    Cancelled,
}

/// Agent scanner configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub max_attempts_per_category: usize,
    pub planner_mode: PlannerMode,
    pub initial_generator_mode: GeneratorMode,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_attempts_per_category: 5,
            planner_mode: PlannerMode::Deterministic,
            initial_generator_mode: GeneratorMode::StaticPack,
        }
    }
}

/// Record of a single phase transition for observability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseRecord {
    pub phase: AgentPhase,
    pub detail: String,
    pub attempt: u32,
    pub retry: u32,
}

/// Verdict summary from judge evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentVerdict {
    pub payload_id: String,
    pub payload_name: String,
    pub vulnerable: bool,
    pub confidence: f32,
    pub summary: String,
}

/// Attack execution summary returned by the host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackExecutionSummary {
    pub category: AttackCategory,
    pub attempts: usize,
    pub verdicts: Vec<AgentVerdict>,
}

impl AttackExecutionSummary {
    pub fn any_vulnerable(&self) -> bool {
        self.verdicts.iter().any(|v| v.vulnerable)
    }
}

/// Result of running the agent loop for one category on one endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryAgentResult {
    pub category: AttackCategory,
    pub attempts: u32,
    pub retries: u32,
    pub vulnerable: bool,
    pub phases: Vec<PhaseRecord>,
    pub stop_reason: AgentStopReason,
    pub findings: u32,
}

/// Aggregated agent scan outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentScanResult {
    pub category_results: Vec<CategoryAgentResult>,
    pub total_attempts: u32,
    pub total_retries: u32,
    pub findings: u32,
    pub summary: String,
}
