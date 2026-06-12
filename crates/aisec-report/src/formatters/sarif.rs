use async_trait::async_trait;
use serde::Serialize;

use crate::error::ReportResult;
use crate::formatters::ReportFormatter;
use crate::types::{GeneratedReport, ReportFormat, ReportFinding, ReportInput, ReportKind, Severity};

const SARIF_SCHEMA: &str =
    "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json";

pub struct SarifFormatter;

#[derive(Serialize)]
struct SarifLog<'a> {
    #[serde(rename = "$schema")]
    schema: &'static str,
    version: &'static str,
    runs: Vec<SarifRun<'a>>,
}

#[derive(Serialize)]
struct SarifRun<'a> {
    tool: SarifTool,
    results: Vec<SarifResult<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    properties: Option<SarifRunProperties<'a>>,
}

#[derive(Serialize)]
struct SarifRunProperties<'a> {
    report_kind: &'static str,
    project_name: &'a str,
    scan_id: &'a str,
}

#[derive(Serialize)]
struct SarifTool {
    driver: SarifDriver,
}

#[derive(Serialize)]
struct SarifDriver {
    name: &'static str,
    version: &'static str,
    information_uri: &'static str,
}

#[derive(Serialize)]
struct SarifResult<'a> {
    rule_id: &'a str,
    level: &'static str,
    message: SarifMessage<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    properties: Option<SarifResultProperties<'a>>,
}

#[derive(Serialize)]
struct SarifMessage<'a> {
    text: &'a str,
}

#[derive(Serialize)]
struct SarifResultProperties<'a> {
    category: &'a str,
    recommendation: Option<&'a str>,
    compliance_refs: &'a [String],
    status: &'a str,
}

#[async_trait]
impl ReportFormatter for SarifFormatter {
    fn format(&self) -> ReportFormat {
        ReportFormat::Sarif
    }

    async fn render(&self, kind: ReportKind, input: &ReportInput) -> ReportResult<GeneratedReport> {
        let results: Vec<SarifResult> = input
            .findings
            .iter()
            .map(|f| finding_to_sarif(f))
            .collect();

        let log = SarifLog {
            schema: SARIF_SCHEMA,
            version: "2.1.0",
            runs: vec![SarifRun {
                tool: SarifTool {
                    driver: SarifDriver {
                        name: "AISec",
                        version: env!("CARGO_PKG_VERSION"),
                        information_uri: "https://github.com/yangyang/aisec",
                    },
                },
                results,
                properties: Some(SarifRunProperties {
                    report_kind: kind.as_str(),
                    project_name: &input.project_name,
                    scan_id: &input.scan_id,
                }),
            }],
        };

        let bytes = serde_json::to_vec_pretty(&log)
            .map_err(|e| crate::error::ReportError::render(e.to_string()))?;

        Ok(GeneratedReport {
            kind,
            format: ReportFormat::Sarif,
            filename: format!("aisec-{}-{}.sarif.json", kind.as_str(), input.scan_id),
            bytes,
            content_type: ReportFormat::Sarif.content_type().into(),
        })
    }
}

fn finding_to_sarif(f: &ReportFinding) -> SarifResult<'_> {
    SarifResult {
        rule_id: &f.category,
        level: severity_to_sarif_level(f.severity),
        message: SarifMessage { text: &f.title },
        properties: Some(SarifResultProperties {
            category: &f.category,
            recommendation: f.recommendation.as_deref(),
            compliance_refs: &f.compliance_refs,
            status: &f.status,
        }),
    }
}

fn severity_to_sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical | Severity::High => "error",
        Severity::Medium => "warning",
        Severity::Low => "note",
        Severity::Info => "none",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::ReportDataBuilder;
    use crate::types::{ReportFinding, Severity};

    #[tokio::test]
    async fn sarif_is_valid_structure() {
        let input = ReportDataBuilder::build(
            "scan-sarif",
            "Proj",
            None,
            vec![ReportFinding {
                id: "f1".into(),
                title: "Injection".into(),
                severity: Severity::Critical,
                category: "prompt_injection".into(),
                description: "d".into(),
                payload: None,
                response: None,
                confidence: None,
                evidence: None,
                recommendation: Some("fix".into()),
                compliance_refs: vec!["LLM01".into()],
                status: "open".into(),
            }],
        );
        let out = SarifFormatter
            .render(ReportKind::Technical, &input)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out.bytes).unwrap();
        assert_eq!(v["version"], "2.1.0");
        assert_eq!(v["runs"][0]["results"][0]["level"], "error");
    }
}
