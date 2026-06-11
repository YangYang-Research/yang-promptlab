use std::collections::HashMap;

use time::OffsetDateTime;

use crate::recommendations::generate_recommendations;
use crate::types::{
    ChartData, Recommendation, ReportFinding, ReportInput, Severity,
};

/// Builds normalized report input from raw findings.
pub struct ReportDataBuilder;

impl ReportDataBuilder {
    pub fn build(
        scan_id: impl Into<String>,
        project_name: impl Into<String>,
        target_name: Option<String>,
        findings: Vec<ReportFinding>,
    ) -> ReportInput {
        let charts = compute_charts(&findings);
        let recommendations = generate_recommendations(&findings);

        ReportInput {
            scan_id: scan_id.into(),
            project_name: project_name.into(),
            target_name,
            generated_at: OffsetDateTime::now_utc(),
            findings,
            recommendations,
            charts,
            metadata: serde_json::json!({}),
        }
    }

    /// Convert storage-layer finding rows into report findings.
    pub fn from_storage_findings(rows: &[StorageFindingRow]) -> Vec<ReportFinding> {
        rows.iter()
            .cloned()
            .map(|r| r.into_report_finding())
            .collect()
    }
}

/// Lightweight adapter for storage finding shape.
#[derive(Debug, Clone)]
pub struct StorageFindingRow {
    pub id: String,
    pub title: String,
    pub severity: String,
    pub category: Option<String>,
    pub description: Option<String>,
    pub evidence_json: Option<String>,
    pub status: String,
}

impl StorageFindingRow {
    pub fn into_report_finding(self) -> ReportFinding {
        let category = self.category.unwrap_or_else(|| "general".into());
        let severity = Severity::from_str_loose(&self.severity);
        let recommendation = crate::recommendations::recommendation_for(&category, severity);

        ReportFinding {
            id: self.id,
            title: self.title,
            severity,
            category: category.clone(),
            description: self.description.unwrap_or_default(),
            evidence: self.evidence_json,
            recommendation: Some(recommendation.description.clone()),
            compliance_refs: crate::recommendations::compliance_refs_for(&category),
            status: self.status,
        }
    }
}

pub fn compute_charts(findings: &[ReportFinding]) -> ChartData {
    let mut severity_map: HashMap<Severity, usize> = HashMap::new();
    let mut category_map: HashMap<String, usize> = HashMap::new();
    let mut risk_score = 0u32;

    for f in findings {
        *severity_map.entry(f.severity).or_insert(0) += 1;
        *category_map.entry(f.category.clone()).or_insert(0) += 1;
        risk_score += f.severity.risk_weight();
    }

    let mut severity_counts: Vec<_> = severity_map.into_iter().collect();
    severity_counts.sort_by_key(|(s, _)| std::cmp::Reverse(*s));

    let mut category_counts: Vec<_> = category_map.into_iter().collect();
    category_counts.sort_by(|a, b| b.1.cmp(&a.1));

    ChartData {
        severity_counts,
        category_counts,
        risk_score,
        total_findings: findings.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_finding(severity: Severity, category: &str) -> ReportFinding {
        ReportFinding {
            id: uuid::Uuid::new_v4().to_string(),
            title: "Test".into(),
            severity,
            category: category.into(),
            description: "desc".into(),
            evidence: None,
            recommendation: None,
            compliance_refs: vec![],
            status: "open".into(),
        }
    }

    #[test]
    fn computes_chart_data() {
        let findings = vec![
            sample_finding(Severity::Critical, "prompt_injection"),
            sample_finding(Severity::High, "jailbreak"),
            sample_finding(Severity::High, "prompt_injection"),
        ];
        let charts = compute_charts(&findings);
        assert_eq!(charts.total_findings, 3);
        assert!(charts.risk_score > 0);
    }
}
