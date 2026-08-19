use async_trait::async_trait;
use serde::Serialize;

use crate::error::ReportResult;
use crate::formatters::ReportFormatter;
use crate::types::{GeneratedReport, ReportFormat, ReportInput, ReportKind};

pub struct JsonFormatter;

#[derive(Serialize)]
struct JsonReport<'a> {
    report_kind: &'static str,
    scan_id: &'a str,
    project_name: &'a str,
    target_name: Option<&'a str>,
    generated_at: String,
    summary: JsonSummary,
    charts: &'a crate::types::ChartData,
    findings: &'a [crate::types::ReportFinding],
    recommendations: &'a [crate::types::Recommendation],
    metadata: &'a serde_json::Value,
}

#[derive(Serialize)]
struct JsonSummary {
    total_findings: usize,
    risk_score: u32,
    by_severity: Vec<SeverityCount>,
}

#[derive(Serialize)]
struct SeverityCount {
    severity: String,
    count: usize,
}

#[async_trait]
impl ReportFormatter for JsonFormatter {
    fn format(&self) -> ReportFormat {
        ReportFormat::Json
    }

    async fn render(&self, kind: ReportKind, input: &ReportInput) -> ReportResult<GeneratedReport> {
        let by_severity = input
            .charts
            .severity_counts
            .iter()
            .map(|(s, c)| SeverityCount {
                severity: s.as_str().into(),
                count: *c,
            })
            .collect();

        let report = JsonReport {
            report_kind: kind.as_str(),
            scan_id: &input.scan_id,
            project_name: &input.project_name,
            target_name: input.target_name.as_deref(),
            generated_at: input.generated_at.to_string(),
            summary: JsonSummary {
                total_findings: input.charts.total_findings,
                risk_score: input.charts.risk_score,
                by_severity,
            },
            charts: &input.charts,
            findings: &input.findings,
            recommendations: &input.recommendations,
            metadata: &input.metadata,
        };

        let bytes = serde_json::to_vec_pretty(&report)
            .map_err(|e| crate::error::ReportError::render(e.to_string()))?;

        Ok(GeneratedReport {
            kind,
            format: ReportFormat::Json,
            filename: format!("promptlab-{}-{}.json", kind.as_str(), input.scan_id),
            bytes,
            content_type: ReportFormat::Json.content_type().into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::ReportDataBuilder;
    use crate::types::{ReportFinding, Severity};

    fn sample_input() -> ReportInput {
        ReportDataBuilder::build(
            "scan-1",
            "Demo Project",
            Some("Chat API".into()),
            vec![ReportFinding {
                id: "f1".into(),
                title: "Prompt injection".into(),
                severity: Severity::High,
                category: "prompt_injection".into(),
                description: "Model leaked policy".into(),
                payload: Some("ignore previous instructions".into()),
                response: Some("policy: ...".into()),
                http_request: None,
                http_response: None,
                confidence: Some(0.8),
                evidence: Some(r#"{"text":"leak"}"#.into()),
                evidence_raw: None,
                recommendation: Some("Add guardrails".into()),
                compliance_refs: vec!["LLM01".into()],
                status: "open".into(),
            }],
        )
    }

    #[tokio::test]
    async fn renders_valid_json() {
        let fmt = JsonFormatter;
        let out = fmt
            .render(ReportKind::Technical, &sample_input())
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out.bytes).unwrap();
        assert_eq!(v["scan_id"], "scan-1");
        assert!(v["findings"].is_array());
    }
}
