use async_trait::async_trait;

use crate::error::ReportResult;
use crate::formatters::ReportFormatter;
use crate::types::{GeneratedReport, ReportFinding, ReportFormat, ReportInput, ReportKind};

pub struct CsvFormatter;

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn finding_row(finding: &ReportFinding) -> String {
    let confidence = finding
        .confidence
        .map(|c| format!("{c:.4}"))
        .unwrap_or_default();
    let compliance = finding.compliance_refs.join("; ");
    [
        finding.id.as_str(),
        finding.title.as_str(),
        finding.severity.as_str(),
        finding.category.as_str(),
        finding.status.as_str(),
        finding.description.as_str(),
        finding.payload.as_deref().unwrap_or(""),
        finding.response.as_deref().unwrap_or(""),
        confidence.as_str(),
        finding.evidence.as_deref().unwrap_or(""),
        finding.recommendation.as_deref().unwrap_or(""),
        compliance.as_str(),
    ]
    .into_iter()
    .map(csv_escape)
    .collect::<Vec<_>>()
    .join(",")
}

#[async_trait]
impl ReportFormatter for CsvFormatter {
    fn format(&self) -> ReportFormat {
        ReportFormat::Csv
    }

    async fn render(&self, kind: ReportKind, input: &ReportInput) -> ReportResult<GeneratedReport> {
        let mut lines = Vec::with_capacity(input.findings.len() + 1);
        lines.push(
            [
                "id",
                "title",
                "severity",
                "category",
                "status",
                "description",
                "payload",
                "response",
                "confidence",
                "evidence",
                "recommendation",
                "compliance_refs",
            ]
            .join(","),
        );

        for finding in &input.findings {
            lines.push(finding_row(finding));
        }

        // Prefix metadata as comment lines so spreadsheets stay findings-first.
        let header = format!(
            "# aisec report_kind={} scan_id={} project={} target={} generated_at={}\n",
            kind.as_str(),
            csv_escape(&input.scan_id),
            csv_escape(&input.project_name),
            csv_escape(input.target_name.as_deref().unwrap_or("")),
            csv_escape(&input.generated_at.to_string()),
        );

        let body = lines.join("\n");
        let bytes = format!("{header}{body}\n").into_bytes();

        Ok(GeneratedReport {
            kind,
            format: ReportFormat::Csv,
            filename: format!("aisec-{}-{}.csv", kind.as_str(), input.scan_id),
            bytes,
            content_type: ReportFormat::Csv.content_type().into(),
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
                title: "Prompt injection, confirmed".into(),
                severity: Severity::High,
                category: "prompt_injection".into(),
                description: "Model leaked \"policy\"".into(),
                payload: Some("ignore previous".into()),
                response: Some("policy: secret".into()),
                confidence: Some(0.875),
                evidence: Some(r#"{"text":"leak"}"#.into()),
                recommendation: Some("Add guardrails".into()),
                compliance_refs: vec!["LLM01".into(), "OWASP".into()],
                status: "open".into(),
            }],
        )
    }

    #[tokio::test]
    async fn renders_csv_with_header_and_escaped_fields() {
        let fmt = CsvFormatter;
        let out = fmt
            .render(ReportKind::Technical, &sample_input())
            .await
            .unwrap();
        let text = String::from_utf8(out.bytes).unwrap();
        assert!(text.contains("id,title,severity,category"));
        assert!(text.contains("\"Prompt injection, confirmed\""));
        assert!(text.contains("\"Model leaked \"\"policy\"\"\""));
        assert!(text.contains("0.8750"));
        assert!(text.contains("LLM01; OWASP"));
        assert_eq!(out.format, ReportFormat::Csv);
        assert!(out.filename.ends_with(".csv"));
    }
}
