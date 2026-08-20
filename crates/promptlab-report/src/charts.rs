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

    pub fn risk_label(score_100: u32) -> &'static str {
        if score_100 >= 75 {
            "Critical"
        } else if score_100 >= 50 {
            "High"
        } else if score_100 >= 25 {
            "Medium"
        } else if score_100 > 0 {
            "Low"
        } else {
            "No detected risk"
        }
    }

    /// HTML bar rows matching Report Details (no overflowing SVG).
    pub fn severity_distribution_html(charts: &ChartData) -> String {
        let counts: Vec<(Severity, usize)> = Severity::all_ordered()
            .iter()
            .map(|sev| {
                let count = charts
                    .severity_counts
                    .iter()
                    .find(|(s, _)| s == sev)
                    .map(|(_, c)| *c)
                    .unwrap_or(0);
                (*sev, count)
            })
            .collect();
        let max = counts.iter().map(|(_, c)| *c).max().unwrap_or(0).max(1);
        let rows: String = counts
            .iter()
            .map(|(sev, count)| {
                let pct = (*count as f64 / max as f64 * 100.0).round() as u32;
                format!(
                    r#"<div class="severity-chart__row">
  <span class="severity-chart__label">{label}</span>
  <div class="severity-chart__bar-track"><div class="severity-chart__bar" style="width:{pct}%;background:{color}"></div></div>
  <span class="severity-chart__count">{count}</span>
</div>"#,
                    label = sev.as_str(),
                    color = html_severity_color(*sev),
                    pct = pct,
                    count = count,
                )
            })
            .collect();
        format!(r#"<div class="severity-chart">{rows}</div>"#)
    }

    /// Doughnut + legend matching in-app Findings by Category.
    pub fn category_doughnut_html(charts: &ChartData) -> String {
        let slices: Vec<_> = charts
            .category_counts
            .iter()
            .filter(|(_, c)| *c > 0)
            .collect();
        if slices.is_empty() {
            return r#"<p class="meta">No findings by attack category yet.</p>"#.into();
        }
        let total: usize = slices.iter().map(|(_, c)| *c).sum();
        let size = 168.0_f64;
        let cx = size / 2.0;
        let cy = size / 2.0;
        let radius = size * 0.36;
        let stroke_width = size * 0.14;
        let circumference = 2.0 * std::f64::consts::PI * radius;
        let mut offset = 0.0;
        let mut circles = String::new();
        let mut legend = String::new();
        for (i, (id, count)) in slices.iter().enumerate() {
            let fraction = *count as f64 / total as f64;
            let length = fraction * circumference;
            let color = category_color(id, i);
            let label = id.replace('_', " ");
            circles.push_str(&format!(
                r#"<circle cx="{cx}" cy="{cy}" r="{radius}" fill="none" stroke="{color}" stroke-width="{sw}" stroke-dasharray="{length:.4} {gap:.4}" stroke-dashoffset="{dash:.4}" transform="rotate(-90 {cx} {cy})"/>"#,
                cx = cx,
                cy = cy,
                radius = radius,
                color = color,
                sw = stroke_width,
                length = length,
                gap = circumference - length,
                dash = -offset,
            ));
            offset += length;
            legend.push_str("<li class=\"category-doughnut__legend-item\"><span class=\"category-doughnut__swatch\" style=\"background:");
            legend.push_str(color);
            legend.push_str("\"></span><span class=\"category-doughnut__legend-label\">");
            legend.push_str(&escape_html(&label));
            legend.push_str("</span><span class=\"category-doughnut__legend-count\">");
            legend.push_str(&count.to_string());
            legend.push_str("</span></li>");
        }
        let mut html = String::new();
        html.push_str(r#"<div class="category-doughnut"><div class="category-doughnut__chart">"#);
        html.push_str(&format!(
            r#"<svg viewBox="0 0 {size} {size}" width="100%" height="auto" role="img" aria-label="Findings by category">"#,
            size = size
        ));
        html.push_str(&format!(
            r#"<circle cx="{cx}" cy="{cy}" r="{radius}" fill="none" stroke="{track}" stroke-width="{sw}"/>"#,
            cx = cx,
            cy = cy,
            radius = radius,
            track = "#ececee",
            sw = stroke_width
        ));
        html.push_str(&circles);
        html.push_str(&format!(
            r#"<text x="{cx}" y="{ty}" text-anchor="middle" font-size="22" font-weight="700" fill="{ink}">{total}</text><text x="{cx}" y="{ly}" text-anchor="middle" font-size="11" fill="{muted}">findings</text></svg></div>"#,
            cx = cx,
            ty = cy - 4.0,
            total = total,
            ly = cy + 14.0,
            ink = "#18181b",
            muted = "#71717a",
        ));
        html.push_str(r#"<ul class="category-doughnut__legend">"#);
        html.push_str(&legend);
        html.push_str("</ul></div>");
        html
    }
}

fn html_severity_color(sev: Severity) -> &'static str {
    match sev {
        Severity::Critical => "#c72929",
        Severity::High => "#f47f1f",
        Severity::Medium => "#ffb300",
        Severity::Low => "#4cae4f",
        Severity::Info => "#1975d2",
    }
}

fn category_color(id: &str, index: usize) -> &'static str {
    match id {
        "prompt_injection" => "#c72929",
        "system_prompt_extraction" => "#f47f1f",
        "jailbreak" => "#eab308",
        "rag_leakage" => "#1975d2",
        "memory_poisoning" => "#0d9488",
        "cross_user_leakage" => "#4cae4f",
        "agent_goal_hijacking" => "#b45309",
        "tool_abuse" => "#64748b",
        "mcp_abuse" => "#0891b2",
        _ => {
            const FALLBACK: &[&str] = &[
                "#c72929", "#f47f1f", "#eab308", "#4cae4f", "#1975d2", "#0d9488", "#b45309",
                "#64748b", "#0891b2", "#78716c",
            ];
            FALLBACK[index % FALLBACK.len()]
        }
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

    #[test]
    fn category_doughnut_uses_circles() {
        let charts = ChartData {
            severity_counts: vec![],
            category_counts: vec![("prompt_injection".into(), 2), ("jailbreak".into(), 1)],
            risk_score: 10,
            total_findings: 3,
        };
        let html = ChartRenderer::category_doughnut_html(&charts);
        assert!(html.contains("category-doughnut"));
        assert!(html.contains("stroke-dasharray"));
        assert!(html.contains("prompt injection"));
    }
}
