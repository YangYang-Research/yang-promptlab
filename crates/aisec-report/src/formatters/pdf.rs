use async_trait::async_trait;
use printpdf::*;
use std::io::BufWriter;

use crate::charts::ChartRenderer;
use crate::error::{ReportError, ReportResult};
use crate::formatters::ReportFormatter;
use crate::types::{GeneratedReport, ReportFormat, ReportFinding, ReportInput, ReportKind, Severity};

pub struct PdfFormatter;

#[async_trait]
impl ReportFormatter for PdfFormatter {
    fn format(&self) -> ReportFormat {
        ReportFormat::Pdf
    }

    async fn render(&self, kind: ReportKind, input: &ReportInput) -> ReportResult<GeneratedReport> {
        let bytes = render_pdf(kind, input)?;
        Ok(GeneratedReport {
            kind,
            format: ReportFormat::Pdf,
            filename: format!("aisec-{}-{}.pdf", kind.as_str(), input.scan_id),
            bytes,
            content_type: ReportFormat::Pdf.content_type().into(),
        })
    }
}

fn render_pdf(kind: ReportKind, input: &ReportInput) -> ReportResult<Vec<u8>> {
    let (doc, page1, layer1) =
        PdfDocument::new(&format!("AISec {}", kind.title()), Mm(210.0), Mm(297.0), "Layer 1");
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| ReportError::render(e.to_string()))?;
    let font_bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| ReportError::render(e.to_string()))?;

    let mut y = 280.0_f32;
    let left = 20.0_f32;
    let line_height = 5.0_f32;

    write_line(
        &doc,
        page1,
        layer1,
        &font_bold,
        left,
        y,
        16.0,
        &format!("{} — {}", kind.title(), input.project_name),
    );
    y -= 10.0;
    write_line(
        &doc,
        page1,
        layer1,
        &font,
        left,
        y,
        10.0,
        &format!(
            "Scan: {} · Generated: {}",
            input.scan_id, input.generated_at
        ),
    );
    y -= 8.0;

    if let Some(target) = &input.target_name {
        write_line(&doc, page1, layer1, &font, left, y, 10.0, &format!("Target: {target}"));
        y -= 8.0;
    }

    y -= 4.0;
    write_line(&doc, page1, layer1, &font_bold, left, y, 12.0, "Executive Summary");
    y -= 7.0;
    let summary = executive_summary_text(kind, input);
    for line in wrap_text(&summary, 85) {
        write_line(&doc, page1, layer1, &font, left, y, 10.0, &line);
        y -= line_height;
    }

    y -= 4.0;
    write_line(
        &doc,
        page1,
        layer1,
        &font_bold,
        left,
        y,
        12.0,
        "Severity Chart",
    );
    y -= 6.0;
    for line in ChartRenderer::severity_text_chart(&input.charts).lines() {
        write_line(&doc, page1, layer1, &font, left, y, 9.0, line);
        y -= 4.5;
    }

    y -= 4.0;
    write_line(
        &doc,
        page1,
        layer1,
        &font_bold,
        left,
        y,
        12.0,
        &format!("Findings ({})", input.findings.len()),
    );
    y -= 7.0;

    for finding in &input.findings {
        if y < 40.0 {
            y = 280.0;
        }
        y = write_finding(&doc, page1, layer1, &font, &font_bold, left, y, kind, finding);
    }

    y -= 4.0;
    if y < 60.0 {
        y = 280.0;
    }
    write_line(
        &doc,
        page1,
        layer1,
        &font_bold,
        left,
        y,
        12.0,
        "Recommendations",
    );
    y -= 7.0;
    for rec in &input.recommendations {
        for line in wrap_text(&format!("[{}] {}", rec.priority.as_str(), rec.title), 85) {
            if y < 30.0 {
                y = 280.0;
            }
            write_line(&doc, page1, layer1, &font, left, y, 9.0, &line);
            y -= 4.5;
        }
        for line in wrap_text(&rec.description, 85) {
            write_line(&doc, page1, layer1, &font, left, y, 8.0, &line);
            y -= 4.0;
        }
        y -= 2.0;
    }

    if kind == ReportKind::Compliance {
        y -= 4.0;
        write_line(
            &doc,
            page1,
            layer1,
            &font_bold,
            left,
            y,
            12.0,
            "Compliance References",
        );
        y -= 7.0;
        for finding in &input.findings {
            for cref in &finding.compliance_refs {
                write_line(
                    &doc,
                    page1,
                    layer1,
                    &font,
                    left,
                    y,
                    8.0,
                    &format!("{} — {}", cref, finding.title),
                );
                y -= 4.0;
            }
        }
    }

    let mut buf = BufWriter::new(Vec::new());
    doc.save(&mut buf)
        .map_err(|e| ReportError::render(e.to_string()))?;
    Ok(buf.into_inner().map_err(|e| ReportError::render(e.to_string()))?)
}

fn write_finding(
    doc: &PdfDocumentReference,
    page: PdfPageIndex,
    layer: PdfLayerIndex,
    font: &IndirectFontRef,
    font_bold: &IndirectFontRef,
    left: f32,
    mut y: f32,
    kind: ReportKind,
    finding: &ReportFinding,
) -> f32 {
    write_line(
        doc,
        page,
        layer,
        font_bold,
        left,
        y,
        10.0,
        &format!("[{}] {}", finding.severity.as_str().to_uppercase(), finding.title),
    );
    y -= 5.0;
    write_line(
        doc,
        page,
        layer,
        font,
        left,
        y,
        9.0,
        &format!("Category: {} · Status: {}", finding.category, finding.status),
    );
    y -= 4.5;

    for line in wrap_text(&finding.description, 85) {
        write_line(doc, page, layer, font, left, y, 9.0, &line);
        y -= 4.5;
    }

    if kind != ReportKind::Executive {
        if let Some(ev) = &finding.evidence {
            let preview = if ev.len() > 120 {
                format!("{}…", &ev[..120])
            } else {
                ev.clone()
            };
            write_line(doc, page, layer, font, left, y, 8.0, &format!("Evidence: {preview}"));
            y -= 4.0;
        }
    }

    if let Some(rec) = &finding.recommendation {
        for line in wrap_text(&format!("Fix: {rec}"), 85) {
            write_line(doc, page, layer, font, left, y, 8.0, &line);
            y -= 4.0;
        }
    }

    y - 3.0
}

fn write_line(
    doc: &PdfDocumentReference,
    page: PdfPageIndex,
    layer: PdfLayerIndex,
    font: &IndirectFontRef,
    x: f32,
    y: f32,
    size: f32,
    text: &str,
) {
    let sanitized = text
        .chars()
        .filter(|c| c.is_ascii())
        .collect::<String>();
    doc.get_page(page)
        .get_layer(layer)
        .use_text(sanitized, size, Mm(x), Mm(y), font);
}

fn executive_summary_text(kind: ReportKind, input: &ReportInput) -> String {
    let critical = input
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Critical)
        .count();
    match kind {
        ReportKind::Executive => format!(
            "{} findings identified. {} critical issues. Risk score: {}.",
            input.charts.total_findings, critical, input.charts.risk_score
        ),
        ReportKind::Technical => format!(
            "Technical report for {} with {} findings and evidence.",
            input.project_name, input.charts.total_findings
        ),
        ReportKind::Compliance => format!(
            "Compliance assessment: {} findings mapped to OWASP LLM Top 10.",
            input.charts.total_findings
        ),
    }
}

fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current = word.to_string();
        } else if current.len() + 1 + word.len() <= max_chars {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current);
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::ReportDataBuilder;
    use crate::types::{ReportFinding, Severity};

    #[tokio::test]
    async fn pdf_has_valid_header() {
        let input = ReportDataBuilder::build(
            "pdf-1",
            "Project",
            None,
            vec![ReportFinding {
                id: "f1".into(),
                title: "Finding".into(),
                severity: Severity::High,
                category: "test".into(),
                description: "Description text".into(),
                payload: None,
                response: None,
                confidence: None,
                evidence: None,
                recommendation: Some("Fix it".into()),
                compliance_refs: vec![],
                status: "open".into(),
            }],
        );
        let out = PdfFormatter
            .render(ReportKind::Executive, &input)
            .await
            .unwrap();
        assert!(out.bytes.starts_with(b"%PDF"));
    }
}
