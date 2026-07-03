//! Typed data-transfer objects returned by IPC commands.
//!
//! Storage row models are mapped into stable, frontend-friendly shapes:
//! timestamps become RFC 3339 strings and JSON text columns become parsed
//! `serde_json::Value`s.

use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use aisec_storage::{Endpoint, Finding, Project, Report, Scan, Target};

fn ts(dt: OffsetDateTime) -> String {
    dt.format(&Rfc3339).unwrap_or_else(|_| dt.to_string())
}

fn ts_opt(dt: Option<OffsetDateTime>) -> Option<String> {
    dt.map(ts)
}

fn json_str(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_string()))
}

fn json_opt(raw: Option<String>) -> Option<serde_json::Value> {
    raw.map(|s| json_str(&s))
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Project> for ProjectDto {
    fn from(p: Project) -> Self {
        Self {
            id: p.id,
            name: p.name,
            description: p.description,
            created_at: ts(p.created_at),
            updated_at: ts(p.updated_at),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TargetDto {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub target_type: String,
    pub descriptor: serde_json::Value,
    pub profile: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Target> for TargetDto {
    fn from(t: Target) -> Self {
        Self {
            id: t.id,
            project_id: t.project_id,
            name: t.name,
            target_type: t.target_type,
            descriptor: json_str(&t.descriptor_json),
            profile: json_str(&t.profile_json),
            created_at: ts(t.created_at),
            updated_at: ts(t.updated_at),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetCapabilitiesDto {
    pub supports_streaming: bool,
    pub supports_tools: bool,
    pub supports_conversation: bool,
    pub supports_attachments: bool,
    pub supports_memory: bool,
    pub supports_agent: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationResultDto {
    pub verified: bool,
    pub verified_at: Option<String>,
    pub provider: String,
    pub model: Option<String>,
    pub capabilities: TargetCapabilitiesDto,
    pub response_time_ms: u64,
    pub status_code: u16,
    pub status: String,
    pub response_preview: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetProfileDto {
    pub provider: String,
    pub framework: String,
    pub method: String,
    pub base_url: String,
    pub path: String,
    pub headers: std::collections::HashMap<String, String>,
    pub request_template: String,
    pub prompt_placeholder: String,
    pub model_field: Option<String>,
    pub streaming_field: Option<String>,
    pub conversation_field: Option<String>,
    pub tool_field: Option<String>,
    pub attachment_field: Option<String>,
    pub default_capabilities: TargetCapabilitiesDto,
    pub verification_strategy: String,
    pub verification: VerificationResultDto,
}

impl From<aisec_target_profile::TargetCapabilities> for TargetCapabilitiesDto {
    fn from(c: aisec_target_profile::TargetCapabilities) -> Self {
        Self {
            supports_streaming: c.supports_streaming,
            supports_tools: c.supports_tools,
            supports_conversation: c.supports_conversation,
            supports_attachments: c.supports_attachments,
            supports_memory: c.supports_memory,
            supports_agent: c.supports_agent,
        }
    }
}

impl From<aisec_target_profile::VerificationResult> for VerificationResultDto {
    fn from(v: aisec_target_profile::VerificationResult) -> Self {
        Self {
            verified: v.verified,
            verified_at: v.verified_at.map(|dt| ts(dt)),
            provider: v.provider,
            model: v.model,
            capabilities: v.capabilities.into(),
            response_time_ms: v.response_time_ms,
            status_code: v.status_code,
            status: v.status,
            response_preview: v.response_preview,
            error_message: v.error_message,
        }
    }
}

impl From<aisec_target_profile::TargetProfile> for TargetProfileDto {
    fn from(p: aisec_target_profile::TargetProfile) -> Self {
        Self {
            provider: p.provider.as_str().into(),
            framework: p.framework,
            method: p.method.as_str().into(),
            base_url: p.base_url,
            path: p.path,
            headers: p.headers,
            request_template: p.request_template,
            prompt_placeholder: p.prompt_placeholder,
            model_field: p.model_field,
            streaming_field: p.streaming_field,
            conversation_field: p.conversation_field,
            tool_field: p.tool_field,
            attachment_field: p.attachment_field,
            default_capabilities: p.default_capabilities.into(),
            verification_strategy: p.verification_strategy,
            verification: p.verification.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationConsoleEntryDto {
    pub method: String,
    pub url: String,
    pub headers: std::collections::HashMap<String, String>,
    pub body: String,
    pub status_code: u16,
    pub response_time_ms: u64,
    pub response_preview: Option<String>,
    pub success: bool,
    pub message: String,
}

impl From<aisec_target_profile::VerificationConsoleEntry> for VerificationConsoleEntryDto {
    fn from(c: aisec_target_profile::VerificationConsoleEntry) -> Self {
        Self {
            method: c.method,
            url: c.url,
            headers: c.headers,
            body: c.body,
            status_code: c.status_code,
            response_time_ms: c.response_time_ms,
            response_preview: c.response_preview,
            success: c.success,
            message: c.message,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanDetailDto {
    pub id: String,
    pub project_id: String,
    pub target_id: Option<String>,
    pub name: String,
    pub status: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub playbook: Option<serde_json::Value>,
}

impl ScanDetailDto {
    pub fn from_scan(scan: Scan) -> Self {
        Self {
            id: scan.id,
            project_id: scan.project_id,
            target_id: scan.target_id,
            name: scan.name,
            status: scan.status,
            started_at: ts_opt(scan.started_at),
            completed_at: ts_opt(scan.completed_at),
            created_at: ts(scan.created_at),
            updated_at: ts(scan.updated_at),
            playbook: json_opt(scan.playbook_json),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanDto {
    pub id: String,
    pub project_id: String,
    pub target_id: Option<String>,
    pub name: String,
    pub status: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Scan> for ScanDto {
    fn from(s: Scan) -> Self {
        Self {
            id: s.id,
            project_id: s.project_id,
            target_id: s.target_id,
            name: s.name,
            status: s.status,
            started_at: ts_opt(s.started_at),
            completed_at: ts_opt(s.completed_at),
            created_at: ts(s.created_at),
            updated_at: ts(s.updated_at),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingDto {
    pub id: String,
    pub scan_id: String,
    pub project_id: String,
    pub target_id: Option<String>,
    pub title: String,
    pub severity: String,
    pub category: Option<String>,
    pub description: Option<String>,
    pub evidence: Option<serde_json::Value>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Finding> for FindingDto {
    fn from(f: Finding) -> Self {
        Self {
            id: f.id,
            scan_id: f.scan_id,
            project_id: f.project_id,
            target_id: f.target_id,
            title: f.title,
            severity: f.severity,
            category: f.category,
            description: f.description,
            evidence: json_opt(f.evidence_json),
            status: f.status,
            created_at: ts(f.created_at),
            updated_at: ts(f.updated_at),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EndpointDto {
    pub id: String,
    pub scan_id: String,
    pub target_id: Option<String>,
    pub url: String,
    pub kind: String,
    pub method: Option<String>,
    pub confidence: f64,
    pub evidence: Option<String>,
    pub source_url: Option<String>,
    pub discovered_at: String,
    pub endpoint_type: String,
    pub ai_framework: Option<String>,
    pub risk_score: u8,
    pub metadata_confidence: f32,
    pub discovery_source: String,
    pub auth_required: bool,
    pub metadata: Option<AiEndpointMetadataDto>,
    pub attack_recommendations: Vec<EndpointAttackRecommendationDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiEndpointMetadataDto {
    pub basic: EndpointBasicDto,
    pub fingerprint: FingerprintMetadataDto,
    pub schema: SchemaMetadataDto,
    pub inference: InferenceFieldsDto,
    pub capabilities: EndpointCapabilitiesDto,
    pub classification: EndpointClassificationDto,
    pub risk: RiskAssessmentDto,
    pub provenance: DiscoveryProvenanceDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<RawObservationDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointBasicDto {
    pub id: String,
    pub url: String,
    pub method: String,
    pub host: String,
    pub protocol: String,
    pub status: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintMetadataDto {
    pub framework: String,
    pub provider: String,
    pub version: String,
    pub confidence: f32,
    pub api_style: String,
    pub technologies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SchemaMetadataDto {
    pub content_type: Option<String>,
    pub request_schema: Option<NormalizedSchemaDto>,
    pub response_schema: Option<NormalizedSchemaDto>,
    pub transport: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedSchemaDto {
    pub format: String,
    pub fields: Vec<SchemaFieldDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaFieldDto {
    pub name: String,
    pub field_type: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InferenceFieldsDto {
    pub prompt_field: Option<String>,
    pub history_field: Option<String>,
    pub conversation_field: Option<String>,
    pub model_field: Option<String>,
    pub stream_field: Option<String>,
    pub tool_field: Option<String>,
    pub attachment_field: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EndpointCapabilitiesDto {
    pub supports_chat: bool,
    pub supports_streaming: bool,
    pub supports_embedding: bool,
    pub supports_vision: bool,
    pub supports_tools: bool,
    pub supports_json_mode: bool,
    pub supports_thinking: bool,
    pub supports_memory: bool,
    pub supports_agent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointClassificationDto {
    pub endpoint_type: String,
    pub ai_framework: String,
    pub confidence: f32,
    pub risk_score: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskAssessmentDto {
    pub score: u8,
    pub factors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryProvenanceDto {
    pub discovery_source: String,
    pub authentication_required: bool,
    pub discovered_at: String,
    pub kind: String,
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawObservationDto {
    pub request_headers: std::collections::HashMap<String, String>,
    pub request_body: Option<String>,
    pub response_headers: std::collections::HashMap<String, String>,
    pub response_body: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointAttackRecommendationDto {
    pub category: String,
    pub reason: String,
    pub priority: u8,
}

impl From<Endpoint> for EndpointDto {
    fn from(e: Endpoint) -> Self {
        let metadata = e
            .metadata_json
            .as_deref()
            .and_then(parse_metadata_json);
        let attack_recommendations =
            stack_fingerprint_from_endpoint(&e)
                .map(|report| {
                    report
                        .attack_recommendations
                        .into_iter()
                        .map(|r| EndpointAttackRecommendationDto {
                            category: r.category,
                            reason: r.reason,
                            priority: r.priority,
                        })
                        .collect()
                })
                .unwrap_or_default();
        Self {
            id: e.id,
            scan_id: e.scan_id,
            target_id: e.target_id,
            url: e.url,
            kind: e.kind,
            method: e.method,
            confidence: e.confidence,
            evidence: e.evidence,
            source_url: e.source_url,
            discovered_at: ts(e.discovered_at),
            endpoint_type: e.endpoint_type,
            ai_framework: e.ai_framework,
            risk_score: e.risk_score.clamp(0, 100) as u8,
            metadata_confidence: e.metadata_confidence as f32,
            discovery_source: e.discovery_source,
            auth_required: e.auth_required != 0,
            metadata,
            attack_recommendations,
        }
    }
}

fn parse_metadata_json(raw: &str) -> Option<AiEndpointMetadataDto> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    serde_json::from_value(value).ok()
}

pub fn stack_fingerprint_from_endpoint(endpoint: &Endpoint) -> Option<aisec_fingerprint::StackFingerprintReport> {
    let metadata = aisec_endpoint_metadata::AiEndpointMetadata::from_json(
        endpoint.metadata_json.as_deref()?,
    )
    .ok()?;
    metadata.stack_fingerprint
}

pub fn metadata_from_endpoint(endpoint: &Endpoint) -> Option<aisec_endpoint_metadata::AiEndpointMetadata> {
    aisec_endpoint_metadata::AiEndpointMetadata::from_json(endpoint.metadata_json.as_deref()?).ok()
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveryStatsDto {
    pub pages_fetched: u64,
    pub pages_failed: u64,
    pub links_extracted: u64,
    pub probes_sent: u64,
    pub duration_ms: u64,
    pub endpoint_count: u64,
    pub errors: Vec<String>,
    #[serde(default)]
    pub phases: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveryRunDto {
    pub scan: ScanDto,
    pub endpoints: Vec<EndpointDto>,
    pub stats: DiscoveryStatsDto,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttackRunDto {
    pub scan: ScanDto,
    pub category: String,
    pub attempts: u64,
    pub successes: u64,
    pub findings: Vec<FindingDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportDto {
    pub id: String,
    pub project_id: String,
    pub scan_id: Option<String>,
    pub name: String,
    pub format: String,
    pub status: String,
    pub file_path: Option<String>,
    pub finding_count: u64,
    pub created_at: String,
    pub updated_at: String,
}

fn finding_count_from_metadata(metadata_json: Option<&str>) -> u64 {
    metadata_json
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .and_then(|v| v.get("findings").and_then(|f| f.as_u64()))
        .unwrap_or(0)
}

impl From<Report> for ReportDto {
    fn from(r: Report) -> Self {
        let finding_count = finding_count_from_metadata(r.metadata_json.as_deref());
        Self {
            id: r.id,
            project_id: r.project_id,
            scan_id: r.scan_id,
            name: r.name,
            format: r.format,
            status: r.status,
            file_path: r.file_path,
            finding_count,
            created_at: ts(r.created_at),
            updated_at: ts(r.updated_at),
        }
    }
}

/// Full report file contents, returned for download/preview.
#[derive(Debug, Clone, Serialize)]
pub struct ReportContentDto {
    pub id: String,
    pub name: String,
    pub format: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanStartDto {
    pub scan_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanStatusDto {
    pub scan_id: String,
    pub status: String,
    pub progress_percent: f64,
    pub completed: u64,
    pub total: u64,
    #[serde(default)]
    pub categories_completed: u64,
    #[serde(default)]
    pub attacks_completed: u64,
    #[serde(default)]
    pub attacks_total: u64,
    #[serde(default)]
    pub testcases_completed: u64,
    #[serde(default)]
    pub testcases_total: u64,
    #[serde(default)]
    pub pause_pending: bool,
    pub findings_count: u64,
    pub current_endpoint: Option<String>,
    pub current_test: Option<String>,
    pub started_at: Option<String>,
    pub agent_mode: bool,
    pub current_phase: Option<String>,
    pub current_attempt: Option<u32>,
    pub current_retry: Option<u32>,
}
