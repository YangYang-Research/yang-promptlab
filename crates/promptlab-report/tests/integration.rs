//! Reporting engine integration tests.

use promptlab_report::{
    ReportDataBuilder, ReportFormat, ReportKind, ReportingEngine, Severity, StorageFindingRow,
};

fn sample_input() -> promptlab_report::ReportInput {
    ReportDataBuilder::build(
        "integration-scan",
        "Integration Project",
        Some("API Target".into()),
        ReportDataBuilder::from_storage_findings(&[
            StorageFindingRow {
                id: "f1".into(),
                title: "Critical injection".into(),
                severity: "critical".into(),
                category: Some("prompt_injection".into()),
                description: Some("System prompt leaked".into()),
                evidence_json: Some(r#"{"leak":true}"#.into()),
                status: "open".into(),
            },
            StorageFindingRow {
                id: "f2".into(),
                title: "RAG source exposure".into(),
                severity: "high".into(),
                category: Some("rag_leakage".into()),
                description: Some("Retrieved documents exposed".into()),
                evidence_json: None,
                status: "open".into(),
            },
        ]),
    )
}

#[tokio::test]
async fn all_formats_produce_output() {
    let dir = tempfile::tempdir().unwrap();
    let engine = ReportingEngine::new(dir.path()).unwrap();
    let input = sample_input();

    for format in [
        ReportFormat::Html,
        ReportFormat::Pdf,
        ReportFormat::Json,
        ReportFormat::Sarif,
        ReportFormat::Csv,
    ] {
        let report = engine
            .generate(ReportKind::Technical, format, &input)
            .await
            .unwrap();
        assert!(!report.bytes.is_empty(), "{format:?}");
        assert!(dir.path().join(&report.filename).exists());
    }
}

#[tokio::test]
async fn compliance_report_includes_mapping() {
    let dir = tempfile::tempdir().unwrap();
    let engine = ReportingEngine::new(dir.path()).unwrap();
    let report = engine
        .generate(
            ReportKind::Compliance,
            ReportFormat::Html,
            &sample_input(),
        )
        .await
        .unwrap();
    let html = String::from_utf8(report.bytes).unwrap();
    assert!(html.contains("Compliance Mapping"));
    assert!(html.contains("OWASP"));
}

#[tokio::test]
async fn executive_html_has_charts() {
    let dir = tempfile::tempdir().unwrap();
    let engine = ReportingEngine::new(dir.path()).unwrap();
    let report = engine
        .generate(ReportKind::Executive, ReportFormat::Html, &sample_input())
        .await
        .unwrap();
    let html = String::from_utf8(report.bytes).unwrap();
    assert!(html.contains("<svg"));
    assert!(html.contains("Executive Summary"));
    assert!(html.contains("Risk score"));
    assert!(html.contains("Total findings"));
    assert!(html.contains("Confirmed"));
    assert!(html.contains("Severity Distribution"));
    assert!(html.contains("Findings by Category"));
}

#[tokio::test]
async fn json_export_has_recommendations() {
    let dir = tempfile::tempdir().unwrap();
    let engine = ReportingEngine::new(dir.path()).unwrap();
    let report = engine
        .generate(ReportKind::Technical, ReportFormat::Json, &sample_input())
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&report.bytes).unwrap();
    assert!(v["recommendations"].as_array().unwrap().len() >= 1);
    assert_eq!(v["summary"]["total_findings"], 2);
}

#[test]
fn severity_and_charts_computed() {
    let input = sample_input();
    assert_eq!(input.charts.total_findings, 2);
    assert!(input.charts.risk_score >= Severity::Critical.risk_weight());
}
