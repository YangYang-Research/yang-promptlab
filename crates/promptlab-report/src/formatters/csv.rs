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
    let request = finding.http_request.as_ref();
    let response = finding.http_response.as_ref();
    let request_method = request.and_then(|r| r.method.as_deref()).unwrap_or("");
    let request_url = request.and_then(|r| r.url.as_deref()).unwrap_or("");
    let request_headers = request
        .map(|r| header_lines(&r.headers))
        .unwrap_or_default();
    let request_body = request.and_then(|r| r.body.as_deref()).unwrap_or("");
    let http_request = request
        .map(crate::evidence::format_http_request)
        .unwrap_or_default();
    let response_status = response
        .and_then(|r| r.status)
        .map(|s| s.to_string())
        .unwrap_or_default();
    let response_headers = response
        .map(|r| header_lines(&r.headers))
        .unwrap_or_default();
    let response_body = response.and_then(|r| r.body.as_deref()).unwrap_or("");
    let http_response = response
        .map(crate::evidence::format_http_response)
        .unwrap_or_default();
    [
        finding.id.as_str(),
        finding.title.as_str(),
        finding.severity.as_str(),
        finding.category.as_str(),
        finding.status.as_str(),
        finding.description.as_str(),
        finding.payload.as_deref().unwrap_or(""),
        finding.response.as_deref().unwrap_or(""),
        request_method,
        request_url,
        request_headers.as_str(),
        request_body,
        http_request.as_str(),
        response_status.as_str(),
        response_headers.as_str(),
        response_body,
        http_response.as_str(),
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

fn header_lines(headers: &std::collections::BTreeMap<String, String>) -> String {
    headers
        .iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect::<Vec<_>>()
        .join("\n")
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
                "request_method",
                "request_url",
                "request_headers",
                "request_body",
                "http_request",
                "response_status",
                "response_headers",
                "response_body",
                "http_response",
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
            "# promptlab report_kind={} scan_id={} project={} target={} generated_at={}\n",
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
            filename: format!("promptlab-{}-{}.csv", kind.as_str(), input.scan_id),
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
                http_request: None,
                http_response: None,
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
        assert!(text.contains("request_method,request_url"));
        assert!(text.contains("http_request,response_status"));
        assert!(text.contains("http_response,confidence"));
        assert!(text.contains("\"Prompt injection, confirmed\""));
        assert!(text.contains("\"Model leaked \"\"policy\"\"\""));
        assert!(text.contains("0.8750"));
        assert!(text.contains("LLM01; OWASP"));
        assert_eq!(out.format, ReportFormat::Csv);
        assert!(out.filename.ends_with(".csv"));
    }

    #[tokio::test]
    async fn csv_includes_full_http_request_and_response() {
        let mut request_headers = std::collections::BTreeMap::new();
        request_headers.insert("Authorization".into(), "[REDACTED]".into());
        request_headers.insert("Content-Type".into(), "application/json".into());
        let mut response_headers = std::collections::BTreeMap::new();
        response_headers.insert("content-type".into(), "application/json".into());
        let input = ReportDataBuilder::build(
            "scan-http",
            "Demo Project",
            None,
            vec![ReportFinding {
                id: "f1".into(),
                title: "Injection".into(),
                severity: Severity::High,
                category: "prompt_injection".into(),
                description: "complied".into(),
                payload: Some("ignore previous".into()),
                response: Some(r#"{"choices":[{"message":{"content":"UNRESTRICTED_OK"}}]}"#.into()),
                http_request: Some(crate::types::ReportHttpRequest {
                    method: Some("POST".into()),
                    url: Some("https://api.example.com/v1/chat".into()),
                    headers: request_headers,
                    body: Some(r#"{"messages":[{"role":"user","content":"ignore previous"}]}"#.into()),
                }),
                http_response: Some(crate::types::ReportHttpResponse {
                    status: Some(200),
                    headers: response_headers,
                    body: Some(r#"{"choices":[{"message":{"content":"UNRESTRICTED_OK"}}]}"#.into()),
                }),
                confidence: Some(0.9),
                evidence: None,
                recommendation: None,
                compliance_refs: vec![],
                status: "open".into(),
            }],
        );
        let text = String::from_utf8(
            CsvFormatter
                .render(ReportKind::Technical, &input)
                .await
                .unwrap()
                .bytes,
        )
        .unwrap();
        assert!(text.contains("POST /v1/chat HTTP/1.1"));
        assert!(text.contains("Authorization: [REDACTED]"));
        assert!(text.contains("HTTP/1.1 200"));
        assert!(text.contains("UNRESTRICTED_OK"));
    }
}
