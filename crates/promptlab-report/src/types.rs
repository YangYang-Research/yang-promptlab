use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Output format for generated reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReportFormat {
    Html,
    Pdf,
    Json,
    Sarif,
    Csv,
}

impl ReportFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Pdf => "pdf",
            Self::Json => "json",
            Self::Sarif => "sarif",
            Self::Csv => "csv",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Pdf => "pdf",
            Self::Json => "json",
            Self::Sarif => "sarif.json",
            Self::Csv => "csv",
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Html => "text/html",
            Self::Pdf => "application/pdf",
            Self::Json => "application/json",
            Self::Sarif => "application/sarif+json",
            Self::Csv => "text/csv",
        }
    }
}

/// Report audience / template style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    Executive,
    Technical,
    Compliance,
}

impl ReportKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Executive => "executive",
            Self::Technical => "technical",
            Self::Compliance => "compliance",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Executive => "Executive Security Summary",
            Self::Technical => "PromptLab - Security Scan Report",
            Self::Compliance => "Compliance Assessment Report",
        }
    }
}

/// Finding severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "critical" => Self::Critical,
            "high" => Self::High,
            "medium" => Self::Medium,
            "low" => Self::Low,
            _ => Self::Info,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    pub fn color(self) -> &'static str {
        match self {
            Self::Info => "#64748b",
            Self::Low => "#22c55e",
            Self::Medium => "#eab308",
            Self::High => "#f97316",
            Self::Critical => "#ef4444",
        }
    }

    pub fn risk_weight(self) -> u32 {
        match self {
            Self::Info => 1,
            Self::Low => 2,
            Self::Medium => 4,
            Self::High => 8,
            Self::Critical => 16,
        }
    }

    pub fn all_ordered() -> &'static [Severity] {
        &[
            Self::Critical,
            Self::High,
            Self::Medium,
            Self::Low,
            Self::Info,
        ]
    }
}

/// A normalized finding for report rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportFinding {
    pub id: String,
    pub title: String,
    pub severity: Severity,
    pub category: String,
    pub description: String,
    /// The attack payload that was sent to the target.
    #[serde(default)]
    pub payload: Option<String>,
    /// The target/model response that was captured.
    #[serde(default)]
    pub response: Option<String>,
    /// Full HTTP request reconstructed from scanner evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_request: Option<ReportHttpRequest>,
    /// Full HTTP response reconstructed from scanner evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_response: Option<ReportHttpResponse>,
    /// Judge confidence score in `[0.0, 1.0]`.
    #[serde(default)]
    pub confidence: Option<f32>,
    pub evidence: Option<String>,
    /// Original scanner evidence JSON (used by HTML detailed findings).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_raw: Option<String>,
    pub recommendation: Option<String>,
    pub compliance_refs: Vec<String>,
    pub status: String,
}

/// Wire HTTP request captured (or reconstructed) for a finding.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportHttpRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

impl ReportHttpRequest {
    pub fn is_empty(&self) -> bool {
        self.method.is_none()
            && self.url.is_none()
            && self.headers.is_empty()
            && self.body.as_deref().map(str::trim).unwrap_or("").is_empty()
    }
}

/// Wire HTTP response captured for a finding.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportHttpResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

impl ReportHttpResponse {
    pub fn is_empty(&self) -> bool {
        self.status.is_none()
            && self.headers.is_empty()
            && self.body.as_deref().map(str::trim).unwrap_or("").is_empty()
    }
}

/// Remediation recommendation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub id: String,
    pub priority: Severity,
    pub title: String,
    pub description: String,
    pub related_findings: Vec<String>,
}

/// Chart dataset for report visualizations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartData {
    pub severity_counts: Vec<(Severity, usize)>,
    pub category_counts: Vec<(String, usize)>,
    pub risk_score: u32,
    pub total_findings: usize,
}

impl ChartData {
    /// Normalized 0–100 risk used by Report Details (`computeRiskScore`).
    pub fn risk_score_100(&self) -> u32 {
        if self.total_findings == 0 {
            return 0;
        }
        let max = (self.total_findings as u32).saturating_mul(Severity::Critical.risk_weight());
        ((self.risk_score as f64 / max.max(1) as f64) * 100.0)
            .min(100.0)
            .round() as u32
    }
}

/// Input data for report generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportInput {
    pub scan_id: String,
    #[serde(default)]
    pub scan_name: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    pub project_name: String,
    #[serde(default)]
    pub target_id: Option<String>,
    pub target_name: Option<String>,
    pub generated_at: OffsetDateTime,
    pub findings: Vec<ReportFinding>,
    pub recommendations: Vec<Recommendation>,
    /// Overview from stored scan AI recommendations, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommendation_overview: Option<String>,
    pub charts: ChartData,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Generated report artifact.
#[derive(Debug, Clone)]
pub struct GeneratedReport {
    pub kind: ReportKind,
    pub format: ReportFormat,
    pub filename: String,
    pub bytes: Vec<u8>,
    pub content_type: String,
}
