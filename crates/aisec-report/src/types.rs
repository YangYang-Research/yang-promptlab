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
}

impl ReportFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Pdf => "pdf",
            Self::Json => "json",
            Self::Sarif => "sarif",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Pdf => "pdf",
            Self::Json => "json",
            Self::Sarif => "sarif.json",
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Html => "text/html",
            Self::Pdf => "application/pdf",
            Self::Json => "application/json",
            Self::Sarif => "application/sarif+json",
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
            Self::Technical => "Technical Findings Report",
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
    pub evidence: Option<String>,
    pub recommendation: Option<String>,
    pub compliance_refs: Vec<String>,
    pub status: String,
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

/// Input data for report generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportInput {
    pub scan_id: String,
    pub project_name: String,
    pub target_name: Option<String>,
    pub generated_at: OffsetDateTime,
    pub findings: Vec<ReportFinding>,
    pub recommendations: Vec<Recommendation>,
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
