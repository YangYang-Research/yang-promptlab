use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::category::AttackCategory;
use crate::lifecycle::AttackPhase;
use crate::payload::MutatorKind;

/// Target surface type for attack routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    LlmApi,
    Chatbot,
    Agent,
    Rag,
    Mcp,
}

/// Endpoint under test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackTarget {
    pub url: String,
    pub kind: TargetKind,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub auth_token: Option<String>,
    /// JSON body template with prompt placeholder for injection.
    pub body_template: Option<String>,
    /// Placeholder token in `body_template` (defaults to `{{PROMPT}}` when unset).
    #[serde(default)]
    pub prompt_placeholder: Option<String>,
    /// Override harness surface (`rest_api`, `openai_compatible`, …).
    #[serde(default)]
    pub harness_surface: Option<String>,
    pub method: Option<String>,
}

impl AttackTarget {
    pub fn llm_api(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            kind: TargetKind::LlmApi,
            headers: HashMap::new(),
            auth_token: None,
            body_template: Some(
                r#"{"model":"gpt-4o","messages":[{"role":"user","content":"{{payload}}"}]}"#
                    .into(),
            ),
            prompt_placeholder: None,
            harness_surface: None,
            method: Some("POST".into()),
        }
    }

    pub fn with_auth(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }
}

/// Maximum in-flight HTTP attack requests per category (pool backfills when one completes).
pub const DEFAULT_ATTACK_CONCURRENCY: usize = 10;

/// Resource limits for a single attack run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackBudget {
    pub max_payloads: usize,
    pub max_mutations_per_payload: usize,
    pub timeout_ms: u64,
    /// Bounded parallelism for outbound probe HTTP requests.
    #[serde(default = "default_attack_concurrency")]
    pub max_concurrent_requests: usize,
    /// Sleep between launching each probe (0 = fire as soon as a pool slot frees).
    #[serde(default)]
    pub inter_request_delay_ms: u64,
}

fn default_attack_concurrency() -> usize {
    DEFAULT_ATTACK_CONCURRENCY
}

impl Default for AttackBudget {
    fn default() -> Self {
        Self {
            max_payloads: 20,
            max_mutations_per_payload: 3,
            timeout_ms: 30_000,
            max_concurrent_requests: DEFAULT_ATTACK_CONCURRENCY,
            inter_request_delay_ms: 0,
        }
    }
}

impl AttackBudget {
    pub fn concurrent_limit(&self) -> usize {
        self.max_concurrent_requests.max(1)
    }
}

/// Runtime context passed through the attack lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackContext {
    pub scan_id: String,
    pub probe_id: String,
    pub target_id: Option<String>,
    pub target: AttackTarget,
    pub budget: AttackBudget,
    /// Payloads produced by `aisec-generator`; override attack builtins per category.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_payloads: Option<HashMap<AttackCategory, Vec<AttackPayload>>>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl AttackContext {
    pub fn new(scan_id: impl Into<String>, probe_id: impl Into<String>, target: AttackTarget) -> Self {
        Self {
            scan_id: scan_id.into(),
            probe_id: probe_id.into(),
            target_id: None,
            target,
            budget: AttackBudget::default(),
            generated_payloads: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_generated_payloads(
        mut self,
        payloads: HashMap<AttackCategory, Vec<AttackPayload>>,
    ) -> Self {
        self.generated_payloads = Some(payloads);
        self
    }
}

/// Payload format for transport encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayloadFormat {
    Plain,
    JsonTemplate,
    MultiTurn,
}

/// Attack payload definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackPayload {
    pub id: String,
    pub name: String,
    pub category: AttackCategory,
    pub content: String,
    pub format: PayloadFormat,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl AttackPayload {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        category: AttackCategory,
        content: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            category,
            content: content.into(),
            format: PayloadFormat::Plain,
            metadata: HashMap::new(),
        }
    }
}

/// Planned attack steps produced during the planning phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackPlan {
    pub attack_id: String,
    pub category: AttackCategory,
    pub mutators: Vec<MutatorKind>,
    pub payload_ids: Vec<String>,
    pub notes: Option<String>,
}

/// Raw response from target transport.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub duration_ms: u64,
    pub normalized: aisec_harness::NormalizedResponse,
}

/// Severity of a successful attack evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FindingSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

/// Evaluation outcome for a single payload attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackEvaluation {
    pub success: bool,
    pub confidence: f32,
    pub severity: Option<FindingSeverity>,
    pub indicators: Vec<String>,
    pub summary: String,
    pub evidence: Option<serde_json::Value>,
}

impl AttackEvaluation {
    pub fn negative(summary: impl Into<String>) -> Self {
        Self {
            success: false,
            confidence: 0.0,
            severity: None,
            indicators: vec![],
            summary: summary.into(),
            evidence: None,
        }
    }

    pub fn positive(
        severity: FindingSeverity,
        confidence: f32,
        summary: impl Into<String>,
        indicators: Vec<String>,
    ) -> Self {
        Self {
            success: true,
            confidence,
            severity: Some(severity),
            indicators,
            summary: summary.into(),
            evidence: None,
        }
    }
}

/// Single payload execution record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadAttempt {
    pub payload_id: String,
    pub payload_name: String,
    pub mutated_content: String,
    pub mutators_applied: Vec<MutatorKind>,
    pub response: AttackResponse,
    pub evaluation: AttackEvaluation,
}

/// Full result of running one attack through its lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackExecutionResult {
    pub attack_id: String,
    pub category: AttackCategory,
    pub probe_id: String,
    pub scan_id: String,
    pub phase: AttackPhase,
    pub attempts: Vec<PayloadAttempt>,
    pub best: Option<AttackEvaluation>,
    pub started_at: OffsetDateTime,
    pub completed_at: OffsetDateTime,
    pub error: Option<String>,
}

impl AttackExecutionResult {
    pub fn any_success(&self) -> bool {
        self.attempts.iter().any(|a| a.evaluation.success)
    }

    pub fn successful_attempts(&self) -> impl Iterator<Item = &PayloadAttempt> {
        self.attempts.iter().filter(|a| a.evaluation.success)
    }
}

/// Aggregated orchestration output across multiple attacks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationReport {
    pub scan_id: String,
    pub results: Vec<AttackExecutionResult>,
    pub findings_count: usize,
    pub started_at: OffsetDateTime,
    pub completed_at: OffsetDateTime,
}
