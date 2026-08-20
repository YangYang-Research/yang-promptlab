use async_trait::async_trait;
use printpdf::image_crate::{self, DynamicImage, ImageBuffer, Rgb as ImageRgb};
use printpdf::path::PaintMode;
use printpdf::*;
use std::io::BufWriter;

use crate::brand::LOGO_PNG;
use crate::charts::ChartRenderer;
use crate::error::{ReportError, ReportResult};
use crate::evidence::{
    format_http_request, format_http_response, parse_finding_detail, parse_http_from_evidence,
};
use crate::formatters::ReportFormatter;
use crate::recommendations::stored_recommendations_from_evidence;
use crate::types::{GeneratedReport, ReportFormat, ReportFinding, ReportInput, ReportKind};

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
            filename: format!("promptlab-{}-{}.pdf", kind.as_str(), input.scan_id),
            bytes,
            content_type: ReportFormat::Pdf.content_type().into(),
        })
    }
}

const PAGE_W_MM: f32 = 210.0;
const PAGE_H_MM: f32 = 297.0;
const COVER_LOGO_MM: f32 = 36.0;
const COVER_LOGO_GAP_MM: f32 = 14.0;
const COVER_EYEBROW_PT: f32 = 18.0;

fn render_pdf(kind: ReportKind, input: &ReportInput) -> ReportResult<Vec<u8>> {
    let (doc, page1, layer1) =
        PdfDocument::new(&format!("PromptLab {}", kind.title()), Mm(PAGE_W_MM), Mm(PAGE_H_MM), "Layer 1");
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| ReportError::render(e.to_string()))?;
    let font_bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| ReportError::render(e.to_string()))?;

    write_cover_page(&doc, page1, layer1, &font, &font_bold, kind, input);
    doc.add_bookmark("Overview", page1);

    let (toc_page, toc_layer) = doc.add_page(Mm(PAGE_W_MM), Mm(PAGE_H_MM), "Layer 1");
    write_toc_page(&doc, toc_page, toc_layer, &font, &font_bold, kind, input);
    doc.add_bookmark("Contents", toc_page);

    let (page, layer) = doc.add_page(Mm(PAGE_W_MM), Mm(PAGE_H_MM), "Layer 1");
    doc.add_bookmark("Summary", page);
    let mut body = PdfBody {
        doc: &doc,
        page,
        layer,
        font: &font,
        font_bold: &font_bold,
        left: BODY_LEFT_MM,
        y: BODY_TOP_MM,
    };

    write_summary_section(&mut body, input);
    write_charts_section(&mut body, input);
    write_executive_summary_section(&mut body, input);
    write_findings_summary_section(&mut body, input);
    write_detailed_findings_section(&mut body, kind, input);

    if kind == ReportKind::Compliance {
        write_compliance_section(&mut body, input);
    }

    write_report_footer(&mut body, kind, input);

    let mut buf = BufWriter::new(Vec::new());
    doc.save(&mut buf)
        .map_err(|e| ReportError::render(e.to_string()))?;
    Ok(buf.into_inner().map_err(|e| ReportError::render(e.to_string()))?)
}

const BODY_LEFT_MM: f32 = 20.0;
const BODY_TOP_MM: f32 = 280.0;
const BODY_BOTTOM_MM: f32 = 24.0;

/// Tracks the current body page and cursor so content never overwrites itself.
struct PdfBody<'a> {
    doc: &'a PdfDocumentReference,
    page: PdfPageIndex,
    layer: PdfLayerIndex,
    font: &'a IndirectFontRef,
    font_bold: &'a IndirectFontRef,
    left: f32,
    y: f32,
}

impl<'a> PdfBody<'a> {
    fn new_page(&mut self) {
        let (page, layer) = self.doc.add_page(Mm(PAGE_W_MM), Mm(PAGE_H_MM), "Layer 1");
        self.page = page;
        self.layer = layer;
        self.y = BODY_TOP_MM;
    }

    fn ensure_space(&mut self, needed_mm: f32) {
        if self.y < BODY_BOTTOM_MM + needed_mm {
            self.new_page();
        }
    }

    fn gap(&mut self, dy: f32) {
        self.y -= dy;
        if self.y < BODY_BOTTOM_MM {
            self.new_page();
        }
    }

    fn write_heading(&mut self, text: &str) {
        self.ensure_space(14.0);
        write_line(
            self.doc,
            self.page,
            self.layer,
            self.font_bold,
            self.left,
            self.y,
            12.0,
            text,
        );
        self.y -= 7.0;
    }

    fn write_text(&mut self, size: f32, text: &str, advance: f32) {
        self.ensure_space(advance.max(4.0));
        write_line(
            self.doc,
            self.page,
            self.layer,
            self.font,
            self.left,
            self.y,
            size,
            text,
        );
        self.y -= advance;
    }

    fn write_bold(&mut self, size: f32, text: &str, advance: f32) {
        self.ensure_space(advance.max(4.0));
        write_line(
            self.doc,
            self.page,
            self.layer,
            self.font_bold,
            self.left,
            self.y,
            size,
            text,
        );
        self.y -= advance;
    }
}

fn write_cover_page(
    doc: &PdfDocumentReference,
    page: PdfPageIndex,
    layer: PdfLayerIndex,
    font: &IndirectFontRef,
    font_bold: &IndirectFontRef,
    kind: ReportKind,
    input: &ReportInput,
) {
    let scan_name = input
        .scan_name
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(input.project_name.as_str());
    let target = input.target_name.as_deref().unwrap_or("—");
    let title_lines = wrap_text(scan_name, 42);
    let project_lines = wrap_text(&input.project_name, 48);
    let scan_id_lines = wrap_text(&input.scan_id, 48);
    let target_lines = wrap_text(target, 48);

    // Vertically center logo + identity block on A4.
    let text_h = 10.0
        + 14.0
        + title_lines.len() as f32 * 10.0
        + 16.0
        + meta_block_height(&project_lines)
        + meta_block_height(&scan_id_lines)
        + meta_block_height(&target_lines);
    let block_h = COVER_LOGO_MM + COVER_LOGO_GAP_MM + text_h;
    let top = ((PAGE_H_MM + block_h) / 2.0).min(PAGE_H_MM - 28.0);

    place_logo(doc, page, layer, (PAGE_W_MM - COVER_LOGO_MM) / 2.0, top - COVER_LOGO_MM, COVER_LOGO_MM);

    let mut y = top - COVER_LOGO_MM - COVER_LOGO_GAP_MM;
    write_line_centered(doc, page, layer, font_bold, y, COVER_EYEBROW_PT, kind.title());
    y -= 14.0;

    for line in &title_lines {
        write_line_centered(doc, page, layer, font_bold, y, 18.0, line);
        y -= 10.0;
    }
    y -= 14.0;

    y = write_cover_meta(doc, page, layer, font, font_bold, y, "Project", &project_lines);
    y = write_cover_meta(doc, page, layer, font, font_bold, y, "Scan ID", &scan_id_lines);
    let _ = write_cover_meta(doc, page, layer, font, font_bold, y, "Target", &target_lines);
}

/// Flatten brand PNG onto opaque black RGB so PDF viewers don't depend on broken SMask soft-masks.
fn logo_rgb_image() -> Option<Image> {
    let img = image_crate::load_from_memory(LOGO_PNG).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut rgb: ImageBuffer<ImageRgb<u8>, Vec<u8>> = ImageBuffer::new(w, h);
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let [r, g, b, a] = pixel.0;
        let alpha = a as f32 / 255.0;
        // Composite over black (matches PromptLab mark background).
        rgb.put_pixel(
            x,
            y,
            ImageRgb([
                (r as f32 * alpha).round() as u8,
                (g as f32 * alpha).round() as u8,
                (b as f32 * alpha).round() as u8,
            ]),
        );
    }
    Some(Image::from_dynamic_image(&DynamicImage::ImageRgb8(rgb)))
}

fn place_logo(
    doc: &PdfDocumentReference,
    page: PdfPageIndex,
    layer: PdfLayerIndex,
    x_mm: f32,
    y_mm: f32,
    size_mm: f32,
) {
    let Some(image) = logo_rgb_image() else {
        return;
    };
    let px = image.image.width.0.max(1) as f32;
    let dpi = px * 25.4 / size_mm;
    image.add_to_layer(
        doc.get_page(page).get_layer(layer),
        ImageTransform {
            translate_x: Some(Mm(x_mm)),
            translate_y: Some(Mm(y_mm)),
            dpi: Some(dpi),
            ..Default::default()
        },
    );
}

/// Body content starts on page 3 (cover=1, contents=2).
const BODY_START_PAGE: u32 = 3;

struct TocEntry {
    label: String,
    /// `None` = section label only (no page number), matching HTML TOC "Findings" header.
    page: Option<u32>,
    child: bool,
}

fn build_toc_entries(kind: ReportKind, input: &ReportInput) -> Vec<TocEntry> {
    let mut entries = vec![
        TocEntry {
            label: "Overview".into(),
            page: Some(1),
            child: false,
        },
        TocEntry {
            label: "Summary".into(),
            page: Some(BODY_START_PAGE),
            child: false,
        },
        TocEntry {
            label: "Charts".into(),
            page: Some(BODY_START_PAGE),
            child: false,
        },
        TocEntry {
            label: "Executive Summary".into(),
            page: Some(BODY_START_PAGE),
            child: false,
        },
        TocEntry {
            label: "Findings Summary".into(),
            page: Some(BODY_START_PAGE),
            child: false,
        },
        TocEntry {
            label: "Detailed Findings".into(),
            page: Some(BODY_START_PAGE),
            child: false,
        },
    ];

    if !input.findings.is_empty() {
        entries.push(TocEntry {
            label: "Findings".into(),
            page: None,
            child: false,
        });
        for (index, finding) in input.findings.iter().enumerate() {
            entries.push(TocEntry {
                label: format!(
                    "Finding #{} · {}",
                    index + 1,
                    toc_label(&finding.title, 48)
                ),
                page: Some(BODY_START_PAGE),
                child: true,
            });
        }
    }

    if kind == ReportKind::Compliance {
        entries.push(TocEntry {
            label: "Compliance Mapping".into(),
            page: Some(BODY_START_PAGE),
            child: false,
        });
    }

    entries
}

fn toc_label(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let short: String = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{short}...")
}

fn write_toc_page(
    doc: &PdfDocumentReference,
    page: PdfPageIndex,
    layer: PdfLayerIndex,
    font: &IndirectFontRef,
    font_bold: &IndirectFontRef,
    kind: ReportKind,
    input: &ReportInput,
) {
    let left = 24.0_f32;
    let right = PAGE_W_MM - 24.0;
    let mut y = 270.0_f32;

    write_line(doc, page, layer, font_bold, left, y, 16.0, "Contents");
    y -= 14.0;

    for entry in build_toc_entries(kind, input) {
        if y < 30.0 {
            // Keep TOC on a single dedicated page; truncate gracefully.
            write_line(
                doc,
                page,
                layer,
                font,
                left,
                y,
                9.0,
                "...",
            );
            break;
        }
        let indent = if entry.child { 10.0 } else { 0.0 };
        let size = if entry.child { 9.0 } else { 11.0 };
        let entry_font = if entry.child || entry.page.is_none() {
            font
        } else {
            font_bold
        };
        write_toc_row(
            doc,
            page,
            layer,
            entry_font,
            left + indent,
            right,
            y,
            size,
            &entry.label,
            entry.page,
        );
        y -= if entry.child { 6.5 } else { 8.0 };
    }
}

fn write_toc_row(
    doc: &PdfDocumentReference,
    page: PdfPageIndex,
    layer: PdfLayerIndex,
    font: &IndirectFontRef,
    left: f32,
    right: f32,
    y: f32,
    size: f32,
    label: &str,
    page_num: Option<u32>,
) {
    let label_sanitized: String = label.chars().filter(|c| c.is_ascii()).collect();
    write_line(doc, page, layer, font, left, y, size, &label_sanitized);

    let Some(page_num) = page_num else {
        return;
    };

    let page_str = page_num.to_string();
    let page_w = approx_text_width_mm(&page_str, size);
    let label_w = approx_text_width_mm(&label_sanitized, size);
    let gap_start = left + label_w + 2.0;
    let gap_end = right - page_w - 2.0;

    if gap_end > gap_start + 4.0 {
        let dot_w = approx_text_width_mm(".", size);
        let mut x = gap_start;
        while x + dot_w < gap_end {
            write_line(doc, page, layer, font, x, y, size, ".");
            x += dot_w.max(1.8);
        }
    }

    write_line(
        doc,
        page,
        layer,
        font,
        right - page_w,
        y,
        size,
        &page_str,
    );
}

fn meta_block_height(value_lines: &[String]) -> f32 {
    5.0 + value_lines.len() as f32 * 7.0 + 10.0
}

fn write_cover_meta(
    doc: &PdfDocumentReference,
    page: PdfPageIndex,
    layer: PdfLayerIndex,
    font: &IndirectFontRef,
    font_bold: &IndirectFontRef,
    mut y: f32,
    label: &str,
    value_lines: &[String],
) -> f32 {
    write_line_centered(doc, page, layer, font, y, 9.0, label);
    y -= 5.0;
    for line in value_lines {
        write_line_centered(doc, page, layer, font_bold, y, 12.0, line);
        y -= 7.0;
    }
    y - 8.0
}

/// Approximate Helvetica text width in mm (avg glyph ≈ 0.5em).
fn approx_text_width_mm(text: &str, size_pt: f32) -> f32 {
    text.chars().count() as f32 * size_pt * 0.5 * 0.352778
}

fn write_line_centered(
    doc: &PdfDocumentReference,
    page: PdfPageIndex,
    layer: PdfLayerIndex,
    font: &IndirectFontRef,
    y: f32,
    size: f32,
    text: &str,
) {
    let sanitized: String = text.chars().filter(|c| c.is_ascii()).collect();
    let x = ((PAGE_W_MM - approx_text_width_mm(&sanitized, size)) / 2.0).max(10.0);
    write_line(doc, page, layer, font, x, y, size, &sanitized);
}

const STAT_CARD_H_MM: f32 = 28.0;
const STAT_CARD_GAP_MM: f32 = 3.0;

fn write_summary_section(body: &mut PdfBody<'_>, input: &ReportInput) {
    let risk_score = input.charts.risk_score_100();
    let risk_label = ChartRenderer::risk_label(risk_score);
    let confirmed = input
        .findings
        .iter()
        .filter(|f| f.status.eq_ignore_ascii_case("confirmed"))
        .count();
    let open = input
        .findings
        .iter()
        .filter(|f| f.status.eq_ignore_ascii_case("open"))
        .count();

    body.write_heading("Summary");
    body.ensure_space(STAT_CARD_H_MM + 8.0);

    let cards = [
        (
            "Risk score",
            format!("{risk_score}/100"),
            Some(risk_label),
        ),
        (
            "Total findings",
            input.findings.len().to_string(),
            None,
        ),
        (
            "Confirmed",
            confirmed.to_string(),
            Some("Validated vulnerabilities"),
        ),
        ("Open", open.to_string(), Some("Awaiting triage")),
    ];
    draw_summary_stat_cards(body, &cards);
    body.y -= STAT_CARD_H_MM + 8.0;
}

fn draw_summary_stat_cards(
    body: &PdfBody<'_>,
    cards: &[(&str, String, Option<&str>); 4],
) {
    let layer = body.doc.get_page(body.page).get_layer(body.layer);
    layer.set_outline_color(Color::Rgb(Rgb::new(0.83, 0.83, 0.87, None)));
    layer.set_outline_thickness(0.7);
    layer.set_fill_color(Color::Rgb(Rgb::new(1.0, 1.0, 1.0, None)));

    let top = body.y;
    let bottom = top - STAT_CARD_H_MM;
    let right = PAGE_W_MM - BODY_LEFT_MM;
    let total_w = right - body.left;
    let cell_w = (total_w - STAT_CARD_GAP_MM * 3.0) / 4.0;

    for (i, (label, value, hint)) in cards.iter().enumerate() {
        let x0 = body.left + i as f32 * (cell_w + STAT_CARD_GAP_MM);
        let x1 = x0 + cell_w;
        let rect = Rect::new(Mm(x0), Mm(bottom), Mm(x1), Mm(top)).with_mode(PaintMode::FillStroke);
        layer.add_rect(rect);

        let pad = 2.8_f32;
        write_line(
            body.doc,
            body.page,
            body.layer,
            body.font,
            x0 + pad,
            top - 5.5,
            7.0,
            label,
        );
        write_line(
            body.doc,
            body.page,
            body.layer,
            body.font_bold,
            x0 + pad,
            top - 14.5,
            14.0,
            value,
        );
        if let Some(hint) = hint {
            write_line(
                body.doc,
                body.page,
                body.layer,
                body.font,
                x0 + pad,
                top - 22.5,
                6.5,
                hint,
            );
        }
    }
}

fn write_charts_section(body: &mut PdfBody<'_>, input: &ReportInput) {
    body.write_heading("Charts");

    body.write_bold(10.0, "Severity Distribution", 5.0);
    let severity_svg = ChartRenderer::severity_distribution_svg(&input.charts, 520);
    place_svg_chart(body, &severity_svg, PAGE_W_MM - BODY_LEFT_MM * 2.0);
    body.gap(6.0);

    body.write_bold(10.0, "Findings by Category", 5.0);
    let category_svg = ChartRenderer::category_doughnut_svg(&input.charts, 520, 210);
    place_svg_chart(body, &category_svg, PAGE_W_MM - BODY_LEFT_MM * 2.0);
    body.gap(6.0);
}

/// Generate chart SVG → convert via printpdf → paint on the current page.
fn place_svg_chart(body: &mut PdfBody<'_>, svg_src: &str, target_width_mm: f32) {
    let Ok(svg) = Svg::parse(svg_src) else {
        body.write_text(8.0, "(chart unavailable)", 4.0);
        return;
    };

    let dpi = 96.0_f32;
    let native_w_pt = svg.width.into_pt(dpi).0.max(1.0);
    let native_h_pt = svg.height.into_pt(dpi).0.max(1.0);
    let target_w_pt: Pt = Mm(target_width_mm).into();
    let scale = target_w_pt.0 / native_w_pt;
    let height_mm = (native_h_pt * scale) * 25.4 / 72.0;

    body.ensure_space(height_mm + 2.0);
    let y_bottom = body.y - height_mm;
    let layer = body.doc.get_page(body.page).get_layer(body.layer);
    let reference = svg.into_xobject(&layer);
    reference.add_to_layer(
        &layer,
        SvgTransform {
            translate_x: Some(Mm(body.left).into()),
            translate_y: Some(Mm(y_bottom).into()),
            scale_x: Some(scale),
            scale_y: Some(scale),
            dpi: Some(dpi),
            ..Default::default()
        },
    );
    body.y = y_bottom - 2.0;
}

fn write_executive_summary_section(body: &mut PdfBody<'_>, input: &ReportInput) {
    body.write_heading("Executive Summary");
    let overview = input
        .recommendation_overview
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let recs = &input.recommendations;

    if overview.is_none() && recs.is_empty() {
        body.write_text(9.0, "No recommendations available yet.", 4.5);
        body.gap(6.0);
        return;
    }

    if let Some(text) = overview {
        for line in wrap_text(text, 85) {
            body.write_text(10.0, &line, 5.0);
        }
        body.gap(2.0);
    }

    for (i, rec) in recs.iter().enumerate() {
        body.write_bold(
            9.0,
            &format!(
                "{:02}. [{}] {}",
                i + 1,
                rec.priority.as_str(),
                rec.title
            ),
            4.5,
        );
        for line in wrap_text(&rec.description, 85) {
            body.write_text(8.0, &line, 4.0);
        }
        body.gap(2.0);
    }
    body.gap(4.0);
}

fn write_findings_summary_section(body: &mut PdfBody<'_>, input: &ReportInput) {
    body.write_heading(&format!("Findings Summary ({})", input.findings.len()));
    if input.findings.is_empty() {
        body.write_text(9.0, "No findings recorded for this scan.", 4.5);
        body.gap(6.0);
        return;
    }

    // Column widths sum to content width (A4 - side margins).
    let col_w = [10.0_f32, 62.0, 22.0, 32.0, 24.0, 20.0];
    let headers = ["No", "Title", "Severity", "Category", "Status", "Confidence"];
    let font_size = 7.5_f32;
    let pad_x = 1.6_f32;
    let pad_y = 2.2_f32;
    let line_h = 3.6_f32;
    let header_h = 8.0_f32;

    let title_max_chars = chars_for_width(col_w[1] - pad_x * 2.0, font_size);
    let rows: Vec<[String; 6]> = input
        .findings
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let conf = f
                .confidence
                .map(|c| format!("{:.0}%", c * 100.0))
                .unwrap_or_else(|| "-".into());
            [
                (i + 1).to_string(),
                f.title.clone(),
                f.severity.as_str().to_string(),
                f.category.replace('_', " "),
                f.status.clone(),
                conf,
            ]
        })
        .collect();

    // Draw header + rows, starting a new page when needed (repeat header).
    let mut row_idx = 0usize;
    while row_idx < rows.len() {
        body.ensure_space(header_h + 12.0);
        let table_left = body.left;
        let table_right = PAGE_W_MM - BODY_LEFT_MM;
        let header_top = body.y;
        let header_bottom = header_top - header_h;

        draw_table_header_row(
            body,
            table_left,
            header_top,
            header_bottom,
            &col_w,
            &headers,
            font_size,
            pad_x,
            pad_y,
        );
        body.y = header_bottom;

        while row_idx < rows.len() {
            let title_lines = wrap_text(&rows[row_idx][1], title_max_chars.max(8));
            let content_lines = title_lines.len().max(1) as f32;
            let row_h = (pad_y * 2.0 + content_lines * line_h).max(7.0);
            if body.y - row_h < BODY_BOTTOM_MM {
                break;
            }
            let top = body.y;
            let bottom = top - row_h;
            draw_table_data_row(
                body,
                table_left,
                top,
                bottom,
                &col_w,
                &rows[row_idx],
                &title_lines,
                font_size,
                pad_x,
                pad_y,
                line_h,
            );
            body.y = bottom;
            row_idx += 1;
        }

        // Outer border for the block drawn on this page.
        let block_top = header_top;
        let block_bottom = body.y;
        stroke_rect(body, table_left, block_bottom, table_right, block_top);
        draw_table_verticals(body, table_left, block_bottom, block_top, &col_w);
    }

    body.gap(8.0);
}

fn chars_for_width(width_mm: f32, size_pt: f32) -> usize {
    let char_w = size_pt * 0.5 * 0.352778;
    ((width_mm / char_w.max(0.1)).floor() as usize).max(4)
}

fn draw_table_header_row(
    body: &PdfBody<'_>,
    left: f32,
    top: f32,
    bottom: f32,
    col_w: &[f32; 6],
    headers: &[&str; 6],
    font_size: f32,
    pad_x: f32,
    pad_y: f32,
) {
    let layer = body.doc.get_page(body.page).get_layer(body.layer);
    layer.set_fill_color(Color::Rgb(Rgb::new(0.96, 0.96, 0.97, None)));
    layer.set_outline_color(Color::Rgb(Rgb::new(0.83, 0.83, 0.87, None)));
    layer.set_outline_thickness(0.5);
    let right = left + col_w.iter().sum::<f32>();
    layer.add_rect(
        Rect::new(Mm(left), Mm(bottom), Mm(right), Mm(top)).with_mode(PaintMode::FillStroke),
    );

    let mut x = left;
    for (i, header) in headers.iter().enumerate() {
        write_line(
            body.doc,
            body.page,
            body.layer,
            body.font_bold,
            x + pad_x,
            top - pad_y - 3.0,
            font_size,
            header,
        );
        x += col_w[i];
    }
    stroke_hline(body, left, right, bottom);
}

fn draw_table_data_row(
    body: &PdfBody<'_>,
    left: f32,
    top: f32,
    bottom: f32,
    col_w: &[f32; 6],
    row: &[String; 6],
    title_lines: &[String],
    font_size: f32,
    pad_x: f32,
    pad_y: f32,
    line_h: f32,
) {
    let right = left + col_w.iter().sum::<f32>();
    stroke_hline(body, left, right, bottom);

    let mut x = left;
    for (i, cell) in row.iter().enumerate() {
        let text_x = x + pad_x;
        if i == 1 {
            let mut y = top - pad_y - 3.0;
            for line in title_lines {
                write_line(
                    body.doc,
                    body.page,
                    body.layer,
                    body.font,
                    text_x,
                    y,
                    font_size,
                    line,
                );
                y -= line_h;
            }
        } else {
            let text = if i == 3 || i == 2 || i == 4 {
                truncate_chars(cell, chars_for_width(col_w[i] - pad_x * 2.0, font_size))
            } else {
                cell.clone()
            };
            write_line(
                body.doc,
                body.page,
                body.layer,
                body.font,
                text_x,
                top - pad_y - 3.0,
                font_size,
                &text,
            );
        }
        x += col_w[i];
    }
}

fn draw_table_verticals(body: &PdfBody<'_>, left: f32, bottom: f32, top: f32, col_w: &[f32; 6]) {
    let mut x = left;
    for w in col_w {
        stroke_vline(body, x, bottom, top);
        x += *w;
    }
    stroke_vline(body, x, bottom, top);
}

fn stroke_rect(body: &PdfBody<'_>, llx: f32, lly: f32, urx: f32, ury: f32) {
    let layer = body.doc.get_page(body.page).get_layer(body.layer);
    layer.set_outline_color(Color::Rgb(Rgb::new(0.83, 0.83, 0.87, None)));
    layer.set_outline_thickness(0.6);
    layer.add_rect(Rect::new(Mm(llx), Mm(lly), Mm(urx), Mm(ury)).with_mode(PaintMode::Stroke));
}

fn stroke_hline(body: &PdfBody<'_>, x0: f32, x1: f32, y: f32) {
    let layer = body.doc.get_page(body.page).get_layer(body.layer);
    layer.set_outline_color(Color::Rgb(Rgb::new(0.83, 0.83, 0.87, None)));
    layer.set_outline_thickness(0.45);
    layer.add_line(Line {
        points: vec![
            (Point::new(Mm(x0), Mm(y)), false),
            (Point::new(Mm(x1), Mm(y)), false),
        ],
        is_closed: false,
    });
}

fn stroke_vline(body: &PdfBody<'_>, x: f32, y0: f32, y1: f32) {
    let layer = body.doc.get_page(body.page).get_layer(body.layer);
    layer.set_outline_color(Color::Rgb(Rgb::new(0.83, 0.83, 0.87, None)));
    layer.set_outline_thickness(0.45);
    layer.add_line(Line {
        points: vec![
            (Point::new(Mm(x), Mm(y0)), false),
            (Point::new(Mm(x), Mm(y1)), false),
        ],
        is_closed: false,
    });
}

fn write_detailed_findings_section(
    body: &mut PdfBody<'_>,
    kind: ReportKind,
    input: &ReportInput,
) {
    // Always begin Detailed Findings on a fresh page.
    body.new_page();
    body.doc.add_bookmark("Detailed Findings", body.page);
    body.write_heading("Detailed Findings");
    if input.findings.is_empty() {
        body.write_text(9.0, "No findings recorded for this scan.", 4.5);
        return;
    }

    let detailed = kind != ReportKind::Executive;
    for (index, finding) in input.findings.iter().enumerate() {
        write_finding(body, index, input, finding, detailed);
    }
}

fn write_compliance_section(body: &mut PdfBody<'_>, input: &ReportInput) {
    body.gap(4.0);
    body.write_heading("Compliance Mapping");
    let mut any = false;
    for finding in &input.findings {
        for cref in &finding.compliance_refs {
            any = true;
            body.write_text(
                8.0,
                &format!(
                    "{} — {} ({})",
                    cref,
                    finding.title,
                    finding.severity.as_str()
                ),
                4.0,
            );
        }
    }
    if !any {
        body.write_text(9.0, "No compliance references recorded.", 4.5);
    }
}

fn write_report_footer(body: &mut PdfBody<'_>, kind: ReportKind, input: &ReportInput) {
    body.gap(10.0);
    body.write_text(
        8.0,
        &format!("PromptLab · Report type: {}", kind.as_str()),
        4.0,
    );
    body.write_text(8.0, &format!("Generated {}", input.generated_at), 4.0);
}

fn truncate_chars(text: &str, max: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max {
        return trimmed.to_string();
    }
    let short: String = trimmed.chars().take(max.saturating_sub(1)).collect();
    format!("{short}.")
}

fn write_finding(
    body: &mut PdfBody<'_>,
    index: usize,
    input: &ReportInput,
    finding: &ReportFinding,
    detailed: bool,
) {
    body.ensure_space(36.0);
    body.write_bold(
        11.0,
        &format!("Finding #{} - {}", index + 1, finding.title),
        6.0,
    );

    body.write_bold(9.0, "Finding Information", 4.5);
    body.write_text(8.0, &format!("Title: {}", finding.title), 4.0);
    body.write_text(8.0, &format!("Project: {}", input.project_name), 4.0);
    body.write_text(8.0, &format!("Scan ID: {}", input.scan_id), 4.0);
    body.write_text(8.0, &format!("Finding ID: {}", finding.id), 4.0);
    body.write_text(
        8.0,
        &format!("Target: {}", input.target_name.as_deref().unwrap_or("-")),
        4.0,
    );

    let detail = parse_finding_detail(finding.evidence_raw.as_deref());
    let (http_request, http_response) =
        if finding.http_request.is_some() || finding.http_response.is_some() {
            (finding.http_request.clone(), finding.http_response.clone())
        } else {
            parse_http_from_evidence(finding.evidence_raw.as_deref().unwrap_or(""))
        };
    let endpoint = http_request
        .as_ref()
        .and_then(|r| r.url.clone())
        .unwrap_or_else(|| "-".into());
    body.write_text(8.0, &format!("Endpoint: {endpoint}"), 4.0);
    body.gap(2.0);

    body.write_bold(9.0, "Assessment", 4.5);
    body.write_text(8.0, &format!("Severity: {}", finding.severity.as_str()), 4.0);
    body.write_text(8.0, &format!("Attack Category: {}", finding.category), 4.0);
    body.write_text(
        8.0,
        &format!("Verdict: {}", detail.verdict.as_deref().unwrap_or("-")),
        4.0,
    );
    body.write_text(8.0, &format!("Status: {}", finding.status), 4.0);
    let compliance = if finding.compliance_refs.is_empty() {
        "-".to_string()
    } else {
        finding.compliance_refs.join(", ")
    };
    body.write_text(8.0, &format!("Compliance: {compliance}"), 4.0);
    let conf = finding
        .confidence
        .map(|c| format!("{:.0}%", c * 100.0))
        .unwrap_or_else(|| "-".into());
    body.write_text(8.0, &format!("Confidence: {conf}"), 4.0);
    body.gap(2.0);

    if detailed {
        write_finding_poc(body, finding, &http_request, &http_response);
        write_finding_recommendations(body, finding);
    }

    body.gap(6.0);
}

fn write_finding_poc(
    body: &mut PdfBody<'_>,
    finding: &ReportFinding,
    http_request: &Option<crate::types::ReportHttpRequest>,
    http_response: &Option<crate::types::ReportHttpResponse>,
) {
    let payload = finding
        .payload
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let has_traffic = http_request.is_some() || http_response.is_some();
    if payload.is_none() && !has_traffic {
        return;
    }

    body.write_bold(9.0, "Proof of Concept (PoC)", 4.5);
    if let Some(p) = payload {
        body.write_text(8.0, "Payload:", 4.0);
        for line in wrap_text(p, 85) {
            body.write_text(8.0, &line, 4.0);
        }
    }
    if has_traffic {
        body.write_text(8.0, "Request:", 4.0);
        let req = http_request
            .as_ref()
            .map(format_http_request)
            .unwrap_or_else(|| "-".into());
        for line in wrap_text(&req, 85) {
            body.write_text(7.0, &line, 3.5);
        }
        body.write_text(8.0, "Response:", 4.0);
        let resp = http_response
            .as_ref()
            .map(format_http_response)
            .unwrap_or_else(|| "-".into());
        for line in wrap_text(&resp, 85) {
            body.write_text(7.0, &line, 3.5);
        }
    }
    body.gap(2.0);
}

fn write_finding_recommendations(body: &mut PdfBody<'_>, finding: &ReportFinding) {
    body.write_bold(9.0, "Recommendations", 4.5);
    match stored_recommendations_from_evidence(finding.evidence_raw.as_deref()) {
        Some((overview, recs)) => {
            for line in wrap_text(&overview, 85) {
                body.write_text(8.0, &line, 4.0);
            }
            for rec in recs {
                body.write_bold(
                    8.0,
                    &format!("[{}] {}", rec.priority.as_str(), rec.title),
                    4.0,
                );
                for line in wrap_text(&rec.description, 85) {
                    body.write_text(8.0, &line, 4.0);
                }
            }
        }
        None => {
            if let Some(rec) = finding
                .recommendation
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                for line in wrap_text(rec, 85) {
                    body.write_text(8.0, &line, 4.0);
                }
            } else {
                body.write_text(8.0, "No recommendations available yet.", 4.0);
            }
        }
    }
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
        let mut input = ReportDataBuilder::build(
            "pdf-1",
            "AI",
            Some("127.0.0.1".into()),
            vec![ReportFinding {
                id: "f1".into(),
                title: "Finding".into(),
                severity: Severity::High,
                category: "test".into(),
                description: "Description text".into(),
                payload: Some("ignore previous".into()),
                response: Some("OK".into()),
                http_request: None,
                http_response: None,
                confidence: Some(0.9),
                evidence: Some(r#"{"indicators":["OK"]}"#.into()),
                evidence_raw: None,
                recommendation: Some("Fix it".into()),
                compliance_refs: vec!["LLM01".into()],
                status: "open".into(),
            }],
        );
        input.scan_name = Some("Agent Scan (custom)".into());
        let out = PdfFormatter
            .render(ReportKind::Technical, &input)
            .await
            .unwrap();
        assert!(out.bytes.starts_with(b"%PDF"));
        assert!(!out.bytes.is_empty());
        assert!(logo_rgb_image().is_some());
        let pdf = String::from_utf8_lossy(&out.bytes);
        assert!(pdf.contains("DeviceRGB"), "cover logo should be DeviceRGB");
        assert!(
            !pdf.contains("/SMask<<"),
            "cover logo must be opaque RGB (no soft-mask stream)"
        );
        // Content streams are Flate-compressed; structural checks cover pagination.
        // Long evidence must paginate instead of overlapping on page 3.
        let many = (0..40)
            .map(|i| format!("line-{i} evidence detail about the attack path"))
            .collect::<Vec<_>>()
            .join(" ");
        input.findings[0].description = many;
        input.findings[0].evidence = Some("x".repeat(2_000));
        let multipage = PdfFormatter
            .render(ReportKind::Technical, &input)
            .await
            .unwrap();
        let pdf_mp = String::from_utf8_lossy(&multipage.bytes);
        let page_count = pdf_mp.matches("/Type/Page").count()
            + pdf_mp.matches("/Type /Page").count();
        assert!(
            page_count >= 4,
            "expected cover+toc+body pages after overflow, got {page_count}"
        );
    }
}
