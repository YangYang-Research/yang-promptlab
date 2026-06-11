use crate::types::{ChartData, Severity};

const SLATE_700: &str = "#334155";
const SLATE_500: &str = "#64748b";
const INDIGO_500: &str = "#6366f1";
const SLATE_200: &str = "#e2e8f0";

/// SVG chart generators for HTML reports.
pub struct ChartRenderer;

impl ChartRenderer {
    /// Horizontal bar chart of findings by severity.
    pub fn severity_bar_svg(charts: &ChartData, width: u32, height: u32) -> String {
        let max = charts
            .severity_counts
            .iter()
            .map(|(_, c)| *c)
            .max()
            .unwrap_or(1)
            .max(1);

        let bar_height = 28;
        let gap = 8;
        let label_width = 70;
        let chart_width = width.saturating_sub(label_width + 20);

        let mut bars = String::new();
        for (i, (severity, count)) in charts.severity_counts.iter().enumerate() {
            let y = 20 + i as u32 * (bar_height + gap);
            let bar_w = (*count as f64 / max as f64 * chart_width as f64) as u32;
            bars.push_str(&format!(
                r#"<text x="0" y="{ty}" class="chart-label">{label}</text>
<text x="{lx}" y="{ty}" class="chart-value">{count}</text>
<rect x="{label_width}" y="{ry}" width="{bar_w}" height="{bar_height}" fill="{color}" rx="4"/>"#,
                ty = y + 18,
                label = severity.as_str(),
                lx = label_width + bar_w + 8,
                count = count,
                ry = y,
                bar_w = bar_w.max(if *count > 0 { 4 } else { 0 }),
                color = severity.color(),
                label_width = label_width,
                bar_height = bar_height,
            ));
        }

        let total_height = 20 + charts.severity_counts.len() as u32 * (bar_height + gap);

        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {vh}" width="{width}" height="{height}" role="img" aria-label="Findings by severity">
<style>.chart-label{{font:12px sans-serif;fill:{slate_700};text-transform:capitalize}}.chart-value{{font:11px sans-serif;fill:{slate_500}}}</style>
<title>Findings by Severity</title>
{bars}
</svg>"#,
            width = width,
            vh = total_height.max(height),
            height = height,
            bars = bars,
            slate_700 = SLATE_700,
            slate_500 = SLATE_500,
        )
    }

    /// Donut-style category breakdown using arcs (simplified as stacked bars).
    pub fn category_bar_svg(charts: &ChartData, width: u32, height: u32) -> String {
        let top: Vec<_> = charts.category_counts.iter().take(6).collect();
        if top.is_empty() {
            return format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}"><text x="10" y="30" font-size="14" fill="{slate_500}">No category data</text></svg>"#,
                slate_500 = SLATE_500,
            );
        }

        let max = top.iter().map(|(_, c)| *c).max().unwrap_or(1).max(1);
        let bar_height = 22;
        let gap = 6;
        let label_width = 120;

        let mut bars = String::new();
        for (i, (cat, count)) in top.iter().enumerate() {
            let y = 16 + i as u32 * (bar_height + gap);
            let bar_w = (*count as f64 / max as f64 * (width - label_width - 40) as f64) as u32;
            let display_cat = if cat.len() > 16 {
                format!("{}…", &cat[..15])
            } else {
                cat.clone()
            };
            bars.push_str(&format!(
                r#"<text x="0" y="{ty}" class="cat-label">{cat}</text>
<rect x="{lw}" y="{y}" width="{bw}" height="{bh}" fill="{bar_color}" opacity="0.85" rx="3"/>
<text x="{tx}" y="{ty}" class="cat-value">{count}</text>"#,
                ty = y + 15,
                cat = escape_xml(&display_cat),
                lw = label_width,
                y = y,
                bw = bar_w.max(4),
                bh = bar_height,
                tx = label_width + bar_w + 6,
                count = count,
                bar_color = INDIGO_500,
            ));
        }

        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="{width}" height="{height}" role="img" aria-label="Findings by category">
<style>.cat-label{{font:11px sans-serif;fill:{slate_700}}}.cat-value{{font:11px sans-serif;fill:{slate_500}}}</style>
<title>Findings by Category</title>
{bars}
</svg>"#,
            width = width,
            height = height,
            bars = bars,
            slate_700 = SLATE_700,
            slate_500 = SLATE_500,
        )
    }

    /// Risk score gauge (0–100 normalized).
    pub fn risk_gauge_svg(risk_score: u32, total: usize, width: u32, height: u32) -> String {
        let max_score = (total * 16).max(1) as f64;
        let pct = ((risk_score as f64 / max_score) * 100.0).min(100.0);
        let angle = pct / 100.0 * 180.0;
        let cx = width / 2;
        let cy = height - 10;
        let r = width.min(height).saturating_sub(40) / 2;

        let color = if pct >= 75.0 {
            "#ef4444"
        } else if pct >= 50.0 {
            "#f97316"
        } else if pct >= 25.0 {
            "#eab308"
        } else {
            "#22c55e"
        };

        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="{width}" height="{height}" role="img" aria-label="Risk score gauge">
<path d="M {x1},{cy} A {r},{r} 0 0 1 {x2},{cy}" fill="none" stroke="{track_color}" stroke-width="12" stroke-linecap="round"/>
<path d="M {x1},{cy} A {r},{r} 0 0 1 {ax},{ay}" fill="none" stroke="{color}" stroke-width="12" stroke-linecap="round"/>
<text x="{cx}" y="{cy}" text-anchor="middle" font-size="28" font-weight="bold" fill="{color}">{pct:.0}%</text>
<text x="{cx}" y="{cy2}" text-anchor="middle" font-size="12" fill="{slate_500}">Risk Score</text>
</svg>"#,
            width = width,
            height = height,
            x1 = cx - r,
            x2 = cx + r,
            cy = cy,
            r = r,
            ax = cx + (r as f64 * (180.0 - angle).to_radians().cos()) as u32,
            ay = cy - (r as f64 * (180.0 - angle).to_radians().sin()) as u32,
            color = color,
            cx = cx,
            pct = pct,
            cy2 = cy + 20,
            track_color = SLATE_200,
            slate_500 = SLATE_500,
        )
    }

    /// Text-based chart for PDF rendering.
    pub fn severity_text_chart(charts: &ChartData) -> String {
        let max = charts
            .severity_counts
            .iter()
            .map(|(_, c)| *c)
            .max()
            .unwrap_or(1)
            .max(1);

        let mut lines = Vec::new();
        for (severity, count) in &charts.severity_counts {
            let bar_len = (*count * 20 / max).max(if *count > 0 { 1 } else { 0 });
            let bar: String = "█".repeat(bar_len);
            lines.push(format!(
                "{:>8} | {bar} ({count})",
                severity.as_str(),
                bar = bar,
                count = count,
            ));
        }
        lines.join("\n")
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn escape_html(s: &str) -> String {
    escape_xml(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ChartData;

    #[test]
    fn severity_svg_contains_bars() {
        let charts = ChartData {
            severity_counts: vec![(Severity::High, 3), (Severity::Low, 1)],
            category_counts: vec![],
            risk_score: 10,
            total_findings: 4,
        };
        let svg = ChartRenderer::severity_bar_svg(&charts, 400, 200);
        assert!(svg.contains("<svg"));
        assert!(svg.contains("#f97316"));
    }
}
