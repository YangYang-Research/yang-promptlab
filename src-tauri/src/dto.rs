//! Typed data-transfer objects returned by IPC commands.
//!
//! Storage row models are mapped into stable, frontend-friendly shapes:
//! timestamps become RFC 3339 strings and JSON text columns become parsed
//! `serde_json::Value`s.

use serde::Serialize;
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
            created_at: ts(t.created_at),
            updated_at: ts(t.updated_at),
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
    pub fingerprint: Option<EndpointFingerprintDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointFingerprintDto {
    pub confidence: f32,
    pub technologies: Vec<FingerprintTechnologyDto>,
    pub agent_frameworks: Vec<FingerprintFrameworkDto>,
    pub ai_components: Vec<FingerprintComponentDto>,
    pub attack_recommendations: Vec<FingerprintRecommendationDto>,
    pub methods_used: Vec<String>,
    pub primary_provider: Option<String>,
    pub api_style: Option<String>,
    pub platform_profile: PlatformProfileDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformProfileDto {
    pub platform: String,
    pub version: String,
    pub auth_type: String,
    pub llm_provider: String,
    pub memory_enabled: bool,
    pub tools_enabled: bool,
    pub rag_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintTechnologyDto {
    pub id: String,
    pub name: String,
    pub category: String,
    pub confidence: f32,
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintFrameworkDto {
    pub id: String,
    pub name: String,
    pub confidence: f32,
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintComponentDto {
    pub id: String,
    pub name: String,
    pub confidence: f32,
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintRecommendationDto {
    pub category: String,
    pub reason: String,
    pub priority: u8,
}

impl From<Endpoint> for EndpointDto {
    fn from(e: Endpoint) -> Self {
        let fingerprint = e.fingerprint_json.as_deref().and_then(parse_fingerprint_json);
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
            fingerprint,
        }
    }
}

fn parse_fingerprint_json(raw: &str) -> Option<EndpointFingerprintDto> {
    let report: aisec_fingerprint::StackFingerprintReport = serde_json::from_str(raw).ok()?;
    Some(EndpointFingerprintDto {
        confidence: report.confidence,
        technologies: report
            .technologies
            .into_iter()
            .map(|t| FingerprintTechnologyDto {
                id: t.id,
                name: t.name,
                category: t.category,
                confidence: t.confidence,
                signals: t.signals,
            })
            .collect(),
        agent_frameworks: report
            .agent_frameworks
            .into_iter()
            .map(|f| FingerprintFrameworkDto {
                id: f.framework.as_str().into(),
                name: f.name,
                confidence: f.confidence,
                signals: f.signals,
            })
            .collect(),
        ai_components: report
            .ai_components
            .into_iter()
            .map(|c| FingerprintComponentDto {
                id: c.component.as_str().into(),
                name: c.name,
                confidence: c.confidence,
                signals: c.signals,
            })
            .collect(),
        attack_recommendations: report
            .attack_recommendations
            .into_iter()
            .map(|r| FingerprintRecommendationDto {
                category: r.category,
                reason: r.reason,
                priority: r.priority,
            })
            .collect(),
        methods_used: report.methods_used,
        primary_provider: report
            .provider_report
            .primary
            .as_ref()
            .map(|p| p.provider.as_str().into()),
        api_style: report.provider_report.primary.as_ref().map(|p| {
            match p.inferred_api_style {
                aisec_fingerprint::ApiStyle::OpenAiCompatible => "openai_compatible",
                aisec_fingerprint::ApiStyle::AnthropicMessages => "anthropic_messages",
                aisec_fingerprint::ApiStyle::GeminiGenerateContent => "gemini_generate_content",
                aisec_fingerprint::ApiStyle::BedrockInvoke => "bedrock_invoke",
                aisec_fingerprint::ApiStyle::OllamaNative => "ollama_native",
                aisec_fingerprint::ApiStyle::Unknown => "unknown",
            }
            .into()
        }),
        platform_profile: PlatformProfileDto {
            platform: report.platform_profile.platform,
            version: report.platform_profile.version,
            auth_type: report.platform_profile.auth_type,
            llm_provider: report.platform_profile.llm_provider,
            memory_enabled: report.platform_profile.memory_enabled,
            tools_enabled: report.platform_profile.tools_enabled,
            rag_enabled: report.platform_profile.rag_enabled,
        },
    })
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
    pub findings_count: u64,
    pub current_endpoint: Option<String>,
    pub current_test: Option<String>,
    pub started_at: Option<String>,
    pub agent_mode: bool,
    pub current_phase: Option<String>,
    pub current_attempt: Option<u32>,
    pub current_retry: Option<u32>,
}
