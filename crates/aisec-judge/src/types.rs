use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Model role in the judge pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelRole {
    /// Determines whether the attack succeeded / vulnerability is present.
    Judge,
    /// Classifies finding category and severity.
    Classifier,
    /// Adversarial analyst — evaluates compliance with attack intent.
    Attacker,
}

impl ModelRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Judge => "judge",
            Self::Classifier => "classifier",
            Self::Attacker => "attacker",
        }
    }

    pub fn all() -> &'static [ModelRole] {
        &[Self::Judge, Self::Classifier, Self::Attacker]
    }
}

impl std::fmt::Display for ModelRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Evaluator backend kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluatorKind {
    Rule,
    Regex,
    Llm,
}

/// Finding severity aligned with attack framework.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "info" => Some(Self::Info),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }

    pub fn weight(self) -> f32 {
        match self {
            Self::Info => 0.2,
            Self::Low => 0.4,
            Self::Medium => 0.6,
            Self::High => 0.8,
            Self::Critical => 1.0,
        }
    }
}

/// Input to the judge engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeRequest {
    pub probe_id: String,
    pub attack_category: String,
    pub payload: String,
    pub response_text: String,
    #[serde(default)]
    pub context: serde_json::Value,
}

/// Result from a single evaluator pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorResult {
    pub evaluator_id: String,
    pub kind: EvaluatorKind,
    pub role: Option<ModelRole>,
    pub vulnerable: bool,
    pub confidence: f32,
    pub severity: Option<Severity>,
    pub category: Option<String>,
    pub rationale: String,
    pub indicators: Vec<String>,
}

/// Consensus aggregation metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusReport {
    pub agreement_ratio: f32,
    pub participating_evaluators: usize,
    pub vulnerable_votes: usize,
    pub dissent: bool,
    pub method: String,
}

/// Final judge verdict with confidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeVerdict {
    pub probe_id: String,
    pub vulnerable: bool,
    pub confidence: f32,
    pub severity: Option<Severity>,
    pub category: Option<String>,
    pub summary: String,
    pub consensus: ConsensusReport,
    pub evaluator_results: Vec<EvaluatorResult>,
    pub judged_at: OffsetDateTime,
}

/// Engine configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeConfig {
    pub enable_rules: bool,
    pub enable_regex: bool,
    pub enable_llm: bool,
    pub consensus_threshold: f32,
    pub min_confidence: f32,
    pub llm_max_tokens: u32,
    pub llm_temperature: f32,
}

impl Default for JudgeConfig {
    fn default() -> Self {
        Self {
            enable_rules: true,
            enable_regex: true,
            enable_llm: true,
            consensus_threshold: 0.55,
            min_confidence: 0.45,
            llm_max_tokens: 512,
            llm_temperature: 0.1,
        }
    }
}
