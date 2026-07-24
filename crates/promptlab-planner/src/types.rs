use promptlab_attack::AttackCategory;
use serde::{Deserialize, Serialize};

/// Planner execution mode stored on generated attack plans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlannerMode {
    Deterministic,
    LocalLlm,
}

/// Generated attack plan for scan execution and payload generation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttackPlan {
    pub mode: PlannerMode,
    pub profile_id: String,
    pub categories: Vec<AttackCategory>,
    pub disabled_tests: Vec<String>,
    pub rationales: Vec<CategoryRationale>,
    pub confidence: f32,
    pub summary: String,
    pub llm_rationale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryRationale {
    pub category: AttackCategory,
    pub reason: String,
    pub priority: u8,
    pub source: String,
}
