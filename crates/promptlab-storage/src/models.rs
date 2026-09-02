use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use time::OffsetDateTime;

// ---------------------------------------------------------------------------
// Row models
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub summary_json: Option<String>,
    /// Persisted health score 0–100 after a completed scan; `None` = N/A.
    pub health_score: Option<i64>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Target {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub target_type: String,
    pub descriptor_json: String,
    pub profile_json: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Scan {
    pub id: String,
    pub project_id: String,
    pub target_id: Option<String>,
    pub name: String,
    pub status: String,
    pub playbook_json: Option<String>,
    pub started_at: Option<OffsetDateTime>,
    pub completed_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Finding {
    pub id: String,
    pub scan_id: String,
    pub project_id: String,
    pub target_id: Option<String>,
    pub title: String,
    pub severity: String,
    pub category: Option<String>,
    pub description: Option<String>,
    pub evidence_json: Option<String>,
    pub status: String,
    pub status_comment: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct Endpoint {
    pub id: String,
    pub scan_id: String,
    pub target_id: Option<String>,
    pub url: String,
    pub kind: String,
    pub method: Option<String>,
    pub confidence: f64,
    pub evidence: Option<String>,
    pub source_url: Option<String>,
    pub discovered_at: OffsetDateTime,
    pub created_at: OffsetDateTime,
    pub metadata_json: Option<String>,
    pub endpoint_type: String,
    pub ai_framework: Option<String>,
    pub risk_score: i64,
    pub metadata_confidence: f64,
    pub discovery_source: String,
    pub auth_required: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Payload {
    pub id: String,
    pub project_id: Option<String>,
    pub name: String,
    pub payload_type: String,
    pub content: String,
    pub metadata_json: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct AttackResult {
    pub id: String,
    pub scan_id: String,
    pub payload_id: Option<String>,
    pub target_id: Option<String>,
    pub probe_id: Option<String>,
    pub success: bool,
    pub response_json: Option<String>,
    pub evaluated_json: Option<String>,
    pub duration_ms: Option<i64>,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Report {
    pub id: String,
    pub project_id: String,
    pub scan_id: Option<String>,
    pub name: String,
    pub format: String,
    pub status: String,
    pub file_path: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct ModelRecord {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub format: String,
    pub file_path: String,
    pub checksum_sha256: Option<String>,
    pub size_bytes: Option<i64>,
    pub verified: bool,
    pub entry_json: String,
    pub metadata_json: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Plugin {
    pub id: String,
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub manifest_json: String,
    pub install_path: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Global attack technique catalog row (editable default prompts).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct AttackCatalogTechnique {
    pub id: String,
    pub category_id: String,
    pub name: String,
    pub description: Option<String>,
    pub content: String,
    pub default_content: String,
    pub tags_json: String,
    pub surface: Option<String>,
    pub owasp: Option<String>,
    pub enabled: bool,
    pub user_modified: bool,
    pub sort_order: i64,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

// ---------------------------------------------------------------------------
// Create / Update DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProject {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateProject {
    pub name: Option<String>,
    pub description: Option<String>,
    pub summary_json: Option<String>,
    /// When `Some`, replaces the stored health score (`None` inside clears it).
    pub health_score: Option<Option<i64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTarget {
    pub project_id: String,
    pub name: String,
    pub target_type: String,
    pub descriptor_json: Option<serde_json::Value>,
    pub profile_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateTarget {
    pub name: Option<String>,
    pub target_type: Option<String>,
    pub descriptor_json: Option<serde_json::Value>,
    pub profile_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateScan {
    pub project_id: String,
    pub target_id: Option<String>,
    pub name: String,
    pub status: Option<String>,
    pub playbook_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateScan {
    pub target_id: Option<Option<String>>,
    pub name: Option<String>,
    pub status: Option<String>,
    pub playbook_json: Option<serde_json::Value>,
    pub started_at: Option<Option<OffsetDateTime>>,
    pub completed_at: Option<Option<OffsetDateTime>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFinding {
    pub scan_id: String,
    pub project_id: String,
    pub target_id: Option<String>,
    pub title: String,
    pub severity: String,
    pub category: Option<String>,
    pub description: Option<String>,
    pub evidence_json: Option<serde_json::Value>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateFinding {
    pub title: Option<String>,
    pub severity: Option<String>,
    pub category: Option<String>,
    pub description: Option<String>,
    pub evidence_json: Option<serde_json::Value>,
    pub status: Option<String>,
    pub status_comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEndpoint {
    pub scan_id: String,
    pub target_id: Option<String>,
    pub url: String,
    pub kind: String,
    pub method: Option<String>,
    pub confidence: f64,
    pub evidence: Option<String>,
    pub source_url: Option<String>,
    pub discovered_at: OffsetDateTime,
    pub metadata_json: Option<String>,
    pub endpoint_type: Option<String>,
    pub ai_framework: Option<String>,
    pub risk_score: Option<i64>,
    pub metadata_confidence: Option<f64>,
    pub discovery_source: Option<String>,
    pub auth_required: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateEndpoint {
    pub method: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePayload {
    pub project_id: Option<String>,
    pub name: String,
    pub payload_type: String,
    pub content: String,
    pub metadata_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdatePayload {
    pub name: Option<String>,
    pub payload_type: Option<String>,
    pub content: Option<String>,
    pub metadata_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAttackResult {
    pub scan_id: String,
    pub payload_id: Option<String>,
    pub target_id: Option<String>,
    pub probe_id: Option<String>,
    pub success: bool,
    pub response_json: Option<serde_json::Value>,
    pub evaluated_json: Option<serde_json::Value>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateAttackResult {
    pub success: Option<bool>,
    pub response_json: Option<serde_json::Value>,
    pub evaluated_json: Option<serde_json::Value>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReport {
    pub project_id: String,
    pub scan_id: Option<String>,
    pub name: String,
    pub format: String,
    pub status: Option<String>,
    pub file_path: Option<String>,
    pub metadata_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateReport {
    pub name: Option<String>,
    pub format: Option<String>,
    pub status: Option<String>,
    pub file_path: Option<String>,
    pub metadata_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateModel {
    pub name: String,
    pub file_path: String,
    pub format: Option<String>,
    pub provider: Option<String>,
    pub checksum_sha256: Option<String>,
    pub size_bytes: Option<i64>,
    pub verified: Option<bool>,
    pub entry_json: Option<String>,
    pub metadata_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateModel {
    pub name: Option<String>,
    pub file_path: Option<String>,
    pub format: Option<String>,
    pub provider: Option<String>,
    pub checksum_sha256: Option<String>,
    pub size_bytes: Option<i64>,
    pub verified: Option<bool>,
    pub entry_json: Option<String>,
    pub metadata_json: Option<serde_json::Value>,
}

/// Full model-registry upsert (denormalized columns + canonical `entry_json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertModelEntry {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub format: String,
    pub file_path: String,
    pub checksum_sha256: Option<String>,
    pub size_bytes: Option<i64>,
    pub verified: bool,
    pub entry_json: String,
    pub metadata_json: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePlugin {
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    pub enabled: Option<bool>,
    pub manifest_json: serde_json::Value,
    pub install_path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdatePlugin {
    pub name: Option<String>,
    pub version: Option<String>,
    pub enabled: Option<bool>,
    pub manifest_json: Option<serde_json::Value>,
    pub install_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertAttackCatalogTechnique {
    pub id: String,
    pub category_id: String,
    pub name: String,
    pub description: Option<String>,
    pub content: String,
    pub default_content: String,
    pub tags_json: String,
    pub surface: Option<String>,
    pub owasp: Option<String>,
    pub enabled: bool,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateAttackCatalogTechnique {
    pub name: Option<String>,
    pub description: Option<String>,
    pub content: Option<String>,
    pub enabled: Option<bool>,
    pub tags_json: Option<String>,
    pub surface: Option<String>,
    pub owasp: Option<String>,
    pub sort_order: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct RuntimeTrafficEvent {
    pub id: String,
    pub at_ms: i64,
    pub direction: String,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRuntimeTrafficEvent {
    pub at_ms: i64,
    pub direction: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct RuntimeTrafficCounters {
    pub id: i64,
    pub lifetime_sent: i64,
    pub lifetime_received: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct JudgeRoleWeights {
    pub id: i64,
    pub judge: f64,
    pub classifier: f64,
    pub attacker: f64,
    pub default_llm: f64,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateJudgeRoleWeights {
    pub judge: f64,
    pub classifier: f64,
    pub attacker: f64,
    pub default_llm: f64,
}

/// App-wide attack mutator allowlist (Advanced → Mutators).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct MutatorSettingsRow {
    pub id: i64,
    pub enabled_mutators_json: String,
    pub category_mutators_json: String,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutatorSettings {
    pub id: i64,
    pub enabled_mutators: Vec<String>,
    /// category_id → ordered mutator ids.
    pub category_mutators: std::collections::BTreeMap<String, Vec<String>>,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMutatorSettings {
    pub enabled_mutators: Vec<String>,
    pub category_mutators: std::collections::BTreeMap<String, Vec<String>>,
}

/// Singleton host hardware profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct HardwareProfileRow {
    pub id: String,
    pub profile_json: String,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareProfileRecord {
    pub id: String,
    pub profile_json: String,
    pub updated_at: OffsetDateTime,
}

/// Generic app setting row (`environment`, `ai_runtime_config`, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct AppSettingRow {
    pub key: String,
    pub value_json: String,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSettingRecord {
    pub key: String,
    pub value_json: String,
    pub updated_at: OffsetDateTime,
}

/// Session-scoped agent working memory (short-term).
///
/// AgentCore-aligned: raw interaction events within one `session_id`
/// (user / assistant / tool / observation), not truncated wire LLM bodies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct AgentShortTermMemory {
    pub id: String,
    pub session_id: String,
    pub agent_id: String,
    pub project_id: Option<String>,
    pub target_id: Option<String>,
    pub scan_id: Option<String>,
    pub role: String,
    pub memory_key: Option<String>,
    pub content: String,
    pub content_json: Option<String>,
    pub importance: f64,
    pub expires_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Aggregated STM session for ListSessions-style browsing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct AgentStmSessionSummary {
    pub session_id: String,
    pub event_count: i64,
    pub first_at: OffsetDateTime,
    pub last_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgentShortTermMemory {
    pub session_id: String,
    pub agent_id: String,
    pub project_id: Option<String>,
    pub target_id: Option<String>,
    pub scan_id: Option<String>,
    pub role: String,
    pub memory_key: Option<String>,
    pub content: String,
    pub content_json: Option<serde_json::Value>,
    pub importance: Option<f64>,
    pub expires_at: Option<OffsetDateTime>,
}

/// Durable agent knowledge (long-term), unique per agent + scope + key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct AgentLongTermMemory {
    pub id: String,
    pub agent_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub memory_key: String,
    pub content: String,
    pub content_json: Option<String>,
    pub importance: f64,
    pub access_count: i64,
    pub last_accessed_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertAgentLongTermMemory {
    pub agent_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub memory_key: String,
    pub content: String,
    pub content_json: Option<serde_json::Value>,
    pub importance: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateAgentLongTermMemory {
    pub content: Option<String>,
    pub content_json: Option<serde_json::Value>,
    pub importance: Option<f64>,
}

/// Helper for serializing optional JSON columns.
pub(crate) fn json_string(value: &Option<serde_json::Value>) -> promptlab_core::PromptLabResult<Option<String>> {
    match value {
        Some(v) => Ok(Some(serde_json::to_string(v).map_err(|err| {
            promptlab_core::PromptLabError::invalid_input(err.to_string())
        })?)),
        None => Ok(None),
    }
}

pub(crate) fn json_string_required(value: &serde_json::Value) -> promptlab_core::PromptLabResult<String> {
    serde_json::to_string(value).map_err(|err| promptlab_core::PromptLabError::invalid_input(err.to_string()))
}
