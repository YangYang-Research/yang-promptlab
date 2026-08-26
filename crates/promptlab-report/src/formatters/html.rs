use async_trait::async_trait;

use crate::charts::{escape_html, ChartRenderer};
use crate::error::ReportResult;
use crate::evidence::{
    format_http_request, format_http_response, parse_finding_detail, parse_http_from_evidence,
    FindingDetailView,
};
use crate::formatters::ReportFormatter;
use crate::types::{
    GeneratedReport, ReportFormat, ReportFinding, ReportInput, ReportKind,
};

pub struct HtmlFormatter;

#[async_trait]
impl ReportFormatter for HtmlFormatter {
    fn format(&self) -> ReportFormat {
        ReportFormat::Html
    }

    async fn render(&self, kind: ReportKind, input: &ReportInput) -> ReportResult<GeneratedReport> {
        let html = render_html(kind, input);
        Ok(GeneratedReport {
            kind,
            format: ReportFormat::Html,
            filename: format!("promptlab-{}-{}.html", kind.as_str(), input.scan_id),
            bytes: html.into_bytes(),
            content_type: ReportFormat::Html.content_type().into(),
        })
    }
}

fn render_html(kind: ReportKind, input: &ReportInput) -> String {
    let risk_score = input.charts.risk_score_100();
    let risk_label = ChartRenderer::risk_label(risk_score);
    let risk_accent = if risk_score >= 50 {
        "critical"
    } else if risk_score >= 25 {
        "warning"
    } else {
        "success"
    };
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
    let stats_html = format!(
        r#"<section class="stats" id="summary" aria-label="Report summary">
  <div class="card stat-card">
    <span class="stat-card__label">Risk score</span>
    <span class="stat-card__value stat-card__value--{accent}">{score}<span class="stat-max">/100</span></span>
    <span class="stat-card__hint">{risk_label}</span>
  </div>
  <div class="card stat-card">
    <span class="stat-card__label">Total findings</span>
    <span class="stat-card__value">{total}</span>
  </div>
  <div class="card stat-card">
    <span class="stat-card__label">Confirmed</span>
    <span class="stat-card__value">{confirmed}</span>
    <span class="stat-card__hint">Validated vulnerabilities</span>
  </div>
  <div class="card stat-card">
    <span class="stat-card__label">Open</span>
    <span class="stat-card__value">{open}</span>
    <span class="stat-card__hint">Awaiting triage</span>
  </div>
</section>"#,
        accent = risk_accent,
        score = risk_score,
        risk_label = risk_label,
        total = input.findings.len(),
        confirmed = confirmed,
        open = open,
    );
    let severity_chart = ChartRenderer::severity_distribution_html(&input.charts);
    let category_chart = ChartRenderer::category_doughnut_html(&input.charts);

    let summary_table = render_findings_summary(&input.findings);
    let findings_html = render_findings(kind, input);
    let compliance_section = if kind == ReportKind::Compliance {
        render_compliance(&input.findings)
    } else {
        String::new()
    };
    let executive_summary = render_executive_summary(input);

    format!(
        r##"<!DOCTYPE html>
<html lang="en" data-theme="light">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>{scan_name} — {title}</title>
<style>
:root {{
  color-scheme: light;
  --bg-app: #f4f4f5;
  --bg-surface: #ffffff;
  --bg-elevated: #fafafa;
  --bg-hover: #ececee;
  --border: #d4d4d8;
  --border-subtle: #e4e4e7;
  --text: #18181b;
  --text-muted: #52525b;
  --text-subtle: #71717a;
  --accent: #0d9488;
  --danger: #dc2626;
  --info: #0284c7;
  --severity-critical: #c72929;
  --severity-high: #f47f1f;
  --severity-medium: #ffb300;
  --severity-low: #4cae4f;
  --severity-info: #1975d2;
  --radius: 12px;
  --font-sans: "Segoe UI Variable", "Segoe UI", ui-sans-serif, system-ui, -apple-system, sans-serif;
  --font-mono: "JetBrains Mono", "Cascadia Code", "SF Mono", ui-monospace, monospace;
}}
* {{ box-sizing: border-box; margin: 0; padding: 0; }}
body {{
  font-family: var(--font-sans);
  background: var(--bg-app);
  color: var(--text);
  line-height: 1.55;
  padding: 1.5rem 1.25rem 3rem;
}}
.layout {{
  display: grid;
  grid-template-columns: minmax(200px, 240px) minmax(0, 1fr);
  gap: 1.5rem;
  max-width: 1280px;
  margin: 0 auto;
  align-items: start;
}}
@media (max-width: 980px) {{
  .layout {{ grid-template-columns: 1fr; }}
  .toc {{ position: static; max-height: none; }}
}}
.toc {{
  position: sticky;
  top: 1rem;
  max-height: calc(100vh - 2rem);
  overflow: auto;
  background: var(--bg-surface);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 1rem 0.9rem;
}}
.toc__title {{
  margin: 0 0 0.75rem;
  font-size: 0.7rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--text-subtle);
}}
.toc__list {{ list-style: none; display: flex; flex-direction: column; gap: 0.15rem; margin: 0; padding: 0; }}
.toc__item--child {{ margin-left: 0.65rem; }}
.toc__link {{
  display: block;
  padding: 0.35rem 0.5rem;
  border-radius: 6px;
  color: var(--text-muted);
  text-decoration: none;
  font-size: 0.8125rem;
  line-height: 1.35;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}}
.toc__link:hover {{ background: var(--bg-hover); color: var(--text); }}
.toc__link--active {{
  background: color-mix(in srgb, var(--accent) 12%, white);
  color: var(--accent);
  font-weight: 650;
}}
.toc__findings-label {{
  margin: 0.55rem 0 0.2rem;
  padding: 0 0.5rem;
  font-size: 0.68rem;
  font-weight: 650;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--text-subtle);
}}
.main {{ min-width: 0; }}
.container {{ max-width: none; margin: 0; }}
#overview, #summary, #charts, #executive-summary, #findings-summary, #detailed-findings, #compliance, .finding-page {{
  scroll-margin-top: 1rem;
}}
header.identity {{
  background: var(--bg-surface);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 1.25rem 1.4rem;
  margin-bottom: 1.25rem;
}}
.identity__brand {{
  display: flex; align-items: center; gap: 0.9rem; margin-bottom: 0.15rem;
}}
.identity__logo {{
  width: 3rem; height: 3rem; flex-shrink: 0; border-radius: 0.7rem;
  display: block; object-fit: cover;
}}
.identity__brand-text {{ min-width: 0; flex: 1; }}
.identity__eyebrow {{
  display: block; margin-bottom: 0.25rem; color: var(--accent);
  font-size: 0.75rem; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase;
}}
.identity__title {{ margin: 0; font-size: 1.5rem; font-weight: 650; letter-spacing: -0.02em; }}
.identity__metadata {{
  display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 0.75rem;
  margin: 0; padding-top: 1rem; border-top: 1px solid var(--border);
}}
@media (max-width: 880px) {{ .identity__metadata {{ grid-template-columns: 1fr; }} }}
.identity__metadata div {{ min-width: 0; }}
.identity__metadata dt {{ margin: 0 0 0.25rem; color: var(--text-muted); font-size: 0.75rem; font-weight: 500; }}
.identity__metadata dd {{ margin: 0; overflow-wrap: anywhere; font-size: 0.9375rem; font-weight: 500; }}
.meta {{ color: var(--text-muted); font-size: 0.875rem; margin-top: 0.35rem; }}
.mono {{ font-family: var(--font-mono); font-size: 0.8rem; }}
.detailed-findings {{ margin-bottom: 1.25rem; }}
.detailed-findings__stack {{ display: flex; flex-direction: column; gap: 1rem; }}
.finding-page {{
  display: flex; flex-direction: column; gap: 1.25rem;
  margin: 0; padding: 1.15rem 1.25rem;
}}
.card.finding-page {{
  background: var(--bg-elevated);
}}
.finding-page__title {{ margin: 0; font-size: 1.125rem; font-weight: 650; letter-spacing: -0.02em; }}
.finding-page .card {{
  background: var(--bg-surface);
}}
.report-footer {{
  display: flex; align-items: baseline; justify-content: space-between; gap: 1rem;
  margin-top: 2rem; color: var(--text-subtle); font-size: 0.75rem;
}}
.report-footer__meta {{ margin: 0; }}
.report-footer__generated {{
  margin: 0 0 0 auto; font-style: italic; color: var(--text-muted); text-align: right;
  white-space: nowrap;
}}
@media (max-width: 640px) {{
  .report-footer {{ flex-direction: column; align-items: flex-end; }}
  .report-footer__meta {{ align-self: flex-start; }}
}}
.badge {{
  display: inline-flex; align-items: center; gap: 0.25rem;
  padding: 0.15rem 0.55rem; border-radius: 999px;
  font-size: 0.7rem; font-weight: 650; text-transform: uppercase; letter-spacing: 0.04em;
  border: 1px solid transparent;
}}
.badge-sev-critical {{ background: color-mix(in srgb, var(--severity-critical) 14%, white); color: var(--severity-critical); }}
.badge-sev-high {{ background: color-mix(in srgb, var(--severity-high) 16%, white); color: #b45309; }}
.badge-sev-medium {{ background: color-mix(in srgb, var(--severity-medium) 22%, white); color: #a16207; }}
.badge-sev-low {{ background: color-mix(in srgb, var(--severity-low) 16%, white); color: #166534; }}
.badge-sev-info {{ background: color-mix(in srgb, var(--severity-info) 14%, white); color: var(--severity-info); }}
.badge-muted {{ background: var(--bg-hover); color: var(--text-muted); }}
.badge-danger {{ background: color-mix(in srgb, var(--danger) 12%, white); color: var(--danger); }}
.badge-info {{ background: color-mix(in srgb, var(--info) 12%, white); color: var(--info); }}
.stats {{ display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 0.75rem; margin-bottom: 1.25rem; }}
.charts {{ display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 1rem; margin-bottom: 1.25rem; align-items: stretch; }}
@media (max-width: 880px) {{ .stats, .charts {{ grid-template-columns: 1fr; }} }}
.stat-card__label {{ display: block; font-size: 0.8125rem; color: var(--text-muted); margin-bottom: 0.25rem; }}
.stat-card__value {{ display: block; font-family: var(--font-mono); font-size: 1.875rem; font-weight: 600; line-height: 1.1; letter-spacing: -0.04em; }}
.stat-card__hint {{ display: block; font-size: 0.75rem; color: var(--text-subtle); margin-top: 0.375rem; }}
.stat-card__value--critical {{ color: var(--severity-critical); }}
.stat-card__value--warning {{ color: #d97706; }}
.stat-card__value--success {{ color: var(--severity-low); }}
.stat-max {{ font-size: 1rem; font-weight: 500; color: var(--text-subtle); margin-left: 0.15rem; }}
.severity-chart__row {{ display: grid; grid-template-columns: 90px minmax(0, 1fr) 24px; align-items: center; gap: 0.75rem; margin-bottom: 0.625rem; }}
.severity-chart__label {{ font-size: 0.8125rem; text-transform: capitalize; color: var(--text-muted); }}
.severity-chart__bar-track {{ height: 8px; background: var(--bg-elevated); border-radius: 4px; overflow: hidden; min-width: 0; }}
.severity-chart__bar {{ height: 100%; border-radius: 4px; }}
.severity-chart__count {{ font-size: 0.8125rem; text-align: right; color: var(--text); }}
.category-doughnut {{ display: flex; gap: 1rem; align-items: center; flex-wrap: wrap; min-width: 0; }}
.category-doughnut__chart {{ width: 168px; max-width: 100%; flex: 0 0 auto; }}
.category-doughnut svg {{ width: 100%; height: auto; display: block; }}
.category-doughnut__legend {{ list-style: none; flex: 1 1 160px; min-width: 0; }}
.category-doughnut__legend-item {{ display: grid; grid-template-columns: 10px minmax(0, 1fr) auto; gap: 0.5rem; align-items: center; margin-bottom: 0.4rem; font-size: 0.8125rem; }}
.category-doughnut__swatch {{ width: 10px; height: 10px; border-radius: 99px; }}
.category-doughnut__legend-label {{ text-transform: capitalize; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
.category-doughnut__legend-count {{ color: var(--text-muted); }}
.card {{
  background: var(--bg-surface);
  border-radius: var(--radius);
  padding: 1.25rem 1.4rem;
  border: 1px solid var(--border);
}}
.card h2, .section-title {{
  font-size: 1.0625rem; font-weight: 650; margin-bottom: 1rem;
}}
.section-head {{ display: flex; align-items: baseline; justify-content: space-between; margin-bottom: 0.75rem; }}
.summary {{ margin-bottom: 1.25rem; color: var(--text-muted); }}
.summary strong {{ color: var(--text); }}
table.summary-table {{ width: 100%; border-collapse: collapse; font-size: 0.875rem; }}
table.summary-table th {{
  text-align: left; font-size: 0.7rem; text-transform: uppercase; letter-spacing: 0.05em;
  color: var(--text-subtle); font-weight: 650; padding: 0.55rem 0.65rem;
  border-bottom: 1px solid var(--border);
}}
table.summary-table td {{
  padding: 0.65rem; border-bottom: 1px solid var(--border-subtle); vertical-align: top;
}}
table.summary-table tbody tr:last-child td {{ border-bottom: none; }}
.compliance-list {{ display: flex; flex-wrap: wrap; gap: 0.4rem; }}
.rec {{
  display: grid; grid-template-columns: 2rem minmax(0, 1fr); gap: 0.65rem 0.75rem;
  padding: 0.85rem 0; border-bottom: 1px solid var(--border-subtle); align-items: start;
}}
.rec:last-child {{ border: none; }}
.rec__index {{
  font-size: 0.8125rem; font-weight: 700; color: var(--text-subtle); letter-spacing: 0.04em;
  line-height: 1.4; padding-top: 0.15rem;
}}
.rec__main {{ min-width: 0; }}
.rec__head {{ display: flex; flex-wrap: wrap; align-items: center; gap: 0.5rem; }}
.rec p {{ color: var(--text-muted); margin-top: 0.25rem; }}
footer {{ margin-top: 2rem; color: var(--text-subtle); font-size: 0.75rem; text-align: center; }}
.finding-page__index {{ font-size: 0.75rem; font-weight: 650; letter-spacing: 0.06em; text-transform: uppercase; color: var(--text-subtle); }}
.detail-section__title {{ font-size: 1.0625rem; font-weight: 650; margin: 0 0 1rem; }}
.finding-details__overview {{ display: grid; grid-template-columns: minmax(0, 1.2fr) minmax(0, 0.8fr); gap: 1.25rem; }}
@media (max-width: 880px) {{ .finding-details__overview {{ grid-template-columns: 1fr; }} }}
.finding-details__meta {{ display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 0.875rem 1.25rem; margin: 0; }}
.finding-details__meta-row {{ display: flex; flex-direction: column; gap: 0.25rem; min-width: 0; }}
.finding-details__meta-row--wide {{ grid-column: 1 / -1; }}
.finding-details__meta dt, .finding-details__signal-label {{ margin: 0; font-size: 0.75rem; font-weight: 500; color: var(--text-subtle); }}
.finding-details__meta dd {{ margin: 0; font-size: 0.9375rem; font-weight: 500; word-break: break-word; }}
.finding-details__signal-grid {{ display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 1rem 1.25rem; }}
.finding-details__signal-item {{ display: flex; flex-direction: column; align-items: flex-start; gap: 0.4rem; min-width: 0; }}
.finding-details__signal-item--confidence {{ grid-column: 1 / -1; gap: 0.55rem; padding-top: 0.25rem; border-top: 1px solid var(--border-subtle); }}
.finding-details__signal-item--compliance {{ grid-column: 1 / -1; }}
.finding-details__confidence-head {{ display: flex; align-items: baseline; justify-content: space-between; width: 100%; }}
.finding-details__confidence-value {{ font-size: 0.8125rem; font-weight: 650; color: var(--accent); }}
.finding-details__confidence-track {{ width: 100%; height: 6px; background: var(--border-subtle); border-radius: 99px; overflow: hidden; }}
.finding-details__confidence-fill {{ height: 100%; background: var(--accent); border-radius: 99px; }}
.finding-detail {{ display: flex; flex-direction: column; gap: 0.875rem; }}
.finding-detail__evidence-stack {{ display: flex; flex-direction: column; gap: 0.875rem; }}
.finding-detail__grid--traffic {{ display: grid; grid-template-columns: minmax(0, 1.1fr) minmax(0, 0.9fr); gap: 0.875rem; }}
@media (max-width: 880px) {{ .finding-detail__grid--traffic {{ grid-template-columns: 1fr; }} }}
.finding-detail__block {{
  border: 1px solid var(--border-subtle); border-radius: 8px;
  background: var(--bg-elevated); padding: 0.875rem 1rem; min-width: 0;
}}
.finding-detail__block-header {{
  display: flex; flex-direction: column; gap: 0.2rem; margin-bottom: 0.625rem;
  padding-bottom: 0.5rem; border-bottom: 1px solid var(--border-subtle);
}}
.finding-detail__block-header h4 {{ margin: 0; font-size: 0.8125rem; font-weight: 600; color: var(--text-subtle); }}
.finding-detail__block-sub {{ font-size: 0.75rem; color: var(--text-muted); word-break: break-all; line-height: 1.4; }}
.finding-detail__code {{
  font-family: var(--font-mono); font-size: 0.75rem; line-height: 1.45;
  white-space: pre-wrap; word-break: break-word; margin: 0; max-height: 28rem; overflow: auto;
}}
.finding-detail__status-code {{
  display: inline-flex; align-items: center; padding: 0.1rem 0.4rem; border-radius: 6px;
  font-family: var(--font-mono); font-size: 0.75rem; font-weight: 650;
}}
.finding-detail__status-code--2xx {{ background: color-mix(in srgb, var(--severity-low) 16%, white); color: #166534; }}
.finding-detail__judge-roles {{ display: grid; gap: 0.55rem; }}
.finding-detail__role {{
  border: 1px solid var(--border-subtle); border-radius: 8px; padding: 0.75rem 0.875rem;
  background: var(--bg-surface); border-left: 3px solid var(--text-subtle);
}}
.finding-detail__role--judge {{ border-left-color: var(--accent); }}
.finding-detail__role--classifier {{ border-left-color: var(--info); }}
.finding-detail__role--attacker {{ border-left-color: #d97706; }}
.finding-detail__role-header {{ display: flex; justify-content: space-between; gap: 0.75rem; align-items: flex-start; }}
.finding-detail__role-name {{ font-weight: 650; font-size: 0.875rem; }}
.finding-detail__role-badges {{ display: flex; flex-wrap: wrap; gap: 0.35rem; align-items: center; }}
.finding-detail__score {{ font-family: var(--font-mono); font-size: 0.8rem; }}
.finding-detail__score-max {{ color: var(--text-subtle); }}
.finding-detail__role-category {{ color: var(--text-muted); font-size: 0.8125rem; margin-top: 0.35rem; }}
.finding-detail__role-rationale {{ color: var(--text); font-size: 0.875rem; margin-top: 0.4rem; line-height: 1.5; }}
.finding-detail__indicators-wrap {{ margin-top: 0.85rem; }}
.finding-detail__indicators-title {{ font-size: 0.75rem; font-weight: 650; color: var(--text-subtle); margin: 0 0 0.5rem; }}
.finding-detail__indicators-grid {{ display: grid; gap: 0.4rem; }}
.finding-detail__indicator {{
  display: grid; grid-template-columns: 1.75rem 1fr; gap: 0.5rem;
  padding: 0.5rem 0.6rem; background: var(--bg-elevated); border-radius: 8px;
}}
.finding-detail__indicator-index {{ font-family: var(--font-mono); font-size: 0.75rem; color: var(--text-subtle); }}
.finding-detail__indicator-role {{ display: block; font-size: 0.7rem; color: var(--text-subtle); margin-bottom: 0.15rem; }}
.finding-detail__indicator-text {{ margin: 0; font-size: 0.875rem; }}
.finding-detail__score-summary {{
  margin-top: 0.9rem; padding-top: 0.75rem; border-top: 1px solid var(--border-subtle);
}}
.finding-detail__score-summary-head {{ display: flex; justify-content: space-between; align-items: center; gap: 0.75rem; }}
.finding-detail__score-label--heading {{ font-size: 0.75rem; font-weight: 650; color: var(--text-subtle); }}
.finding-detail__score--summary .finding-detail__score-value {{ font-size: 1.25rem; font-weight: 700; }}
.finding-detail__consensus {{ margin: 0.5rem 0 0; color: var(--text-muted); font-size: 0.8125rem; }}
.back-to-top {{
  position: fixed; right: 1.25rem; bottom: 1.25rem; z-index: 50;
  width: 2.5rem; height: 2.5rem; padding: 0; border-radius: 999px;
  border: 1px solid var(--border); background: color-mix(in srgb, var(--bg-surface) 92%, transparent);
  color: var(--text); box-shadow: 0 8px 24px rgba(24, 24, 27, 0.12);
  display: inline-flex; align-items: center; justify-content: center; cursor: pointer;
  opacity: 0; pointer-events: none; transform: translateY(0.5rem);
  transition: opacity 180ms ease, transform 180ms ease, background 120ms ease, border-color 120ms ease;
  backdrop-filter: blur(10px); -webkit-backdrop-filter: blur(10px);
}}
.back-to-top:hover {{ background: var(--bg-hover); border-color: var(--accent); color: var(--accent); }}
.back-to-top--visible {{ opacity: 1; pointer-events: auto; transform: none; }}
.back-to-top svg {{ width: 1.125rem; height: 1.125rem; display: block; }}
</style>
</head>
<body>
<div class="layout">
<nav class="toc" aria-label="Table of contents">
  <p class="toc__title">Contents</p>
  {toc_html}
</nav>
<div class="main">
<div class="container">
<header class="identity card" id="overview">
  <div class="identity__brand">
    <img class="identity__logo" src="{logo_src}" alt="PromptLab" width="48" height="48" />
    <div class="identity__brand-text">
      <span class="identity__eyebrow">{title}</span>
      <h1 class="identity__title">{scan_name}</h1>
    </div>
  </div>
  <dl class="identity__metadata">
    <div><dt>Project</dt><dd>{project}</dd></div>
    <div><dt>Scan ID</dt><dd class="mono">{scan_id}</dd></div>
    <div><dt>Target</dt><dd>{target}</dd></div>
  </dl>
</header>

{stats_html}

<section class="charts" id="charts">
  <div class="card"><h2>Severity Distribution</h2>{severity_chart}</div>
  <div class="card"><h2>Findings by Category</h2>{category_chart}</div>
</section>

{executive_summary}

<section class="card" id="findings-summary" style="margin-bottom:1.25rem">
  <h2>Findings Summary ({finding_count})</h2>
  {summary_table}
</section>

<section class="card detailed-findings" id="detailed-findings">
  <h2>Detailed Findings</h2>
  <div class="detailed-findings__stack">{findings_html}</div>
</section>

{compliance_section}

<footer class="report-footer">
  <p class="report-footer__meta">PromptLab · Report type: {kind}</p>
  <time class="report-footer__generated" id="report-generated" datetime="{generated_iso}">Generated {generated}</time>
</footer>
</div>
</div>
</div>
<button type="button" class="back-to-top" id="back-to-top" aria-label="Back to top" title="Back to top">
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 19V5"/><path d="m5 12 7-7 7 7"/></svg>
</button>
<script>
(function () {{
  var gen = document.getElementById("report-generated");
  if (gen) {{
    var iso = gen.getAttribute("datetime");
    if (iso) {{
      var d = new Date(iso);
      if (!isNaN(d.getTime())) {{
        gen.textContent = "Generated " + d.toLocaleString(undefined, {{
          year: "numeric",
          month: "2-digit",
          day: "2-digit",
          hour: "2-digit",
          minute: "2-digit",
          second: "2-digit",
          hour12: false,
          timeZoneName: "short"
        }});
      }}
    }}
  }}

  var tocLinks = Array.prototype.slice.call(document.querySelectorAll(".toc__link"));
  var sections = tocLinks
    .map(function (link) {{
      var id = (link.getAttribute("href") || "").replace(/^#/, "");
      return id ? document.getElementById(id) : null;
    }})
    .filter(Boolean);

  function setActive(id) {{
    tocLinks.forEach(function (link) {{
      var active = link.getAttribute("href") === "#" + id;
      link.classList.toggle("toc__link--active", active);
    }});
  }}

  if (sections.length && "IntersectionObserver" in window) {{
    var visible = new Map();
    var observer = new IntersectionObserver(function (entries) {{
      entries.forEach(function (entry) {{
        visible.set(entry.target.id, entry.isIntersecting ? entry.intersectionRatio : 0);
      }});
      var bestId = null;
      var bestRatio = 0;
      visible.forEach(function (ratio, id) {{
        if (ratio > bestRatio) {{
          bestRatio = ratio;
          bestId = id;
        }}
      }});
      if (bestId) setActive(bestId);
    }}, {{ rootMargin: "-12% 0px -70% 0px", threshold: [0, 0.1, 0.25, 0.5, 1] }});
    sections.forEach(function (section) {{ observer.observe(section); }});
  }}

  tocLinks.forEach(function (link) {{
    link.addEventListener("click", function () {{
      var id = (link.getAttribute("href") || "").replace(/^#/, "");
      if (id) setActive(id);
    }});
  }});

  var btn = document.getElementById("back-to-top");
  if (!btn) return;
  var showAfter = 240;
  function update() {{
    if (window.scrollY >= showAfter) btn.classList.add("back-to-top--visible");
    else btn.classList.remove("back-to-top--visible");
  }}
  btn.addEventListener("click", function () {{
    window.scrollTo({{ top: 0, behavior: "smooth" }});
  }});
  window.addEventListener("scroll", update, {{ passive: true }});
  update();
}})();
</script>
</body>
</html>"##,
        title = kind.title(),
        logo_src = crate::brand::logo_data_uri(),
        scan_name = escape_html(
            input
                .scan_name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(input.project_name.as_str()),
        ),
        project = escape_html(&input.project_name),
        scan_id = escape_html(&input.scan_id),
        generated = escape_html(&format_generated_at(input.generated_at)),
        generated_iso = escape_html(&format_generated_iso(input.generated_at)),
        target = escape_html(input.target_name.as_deref().unwrap_or("—")),
        toc_html = render_toc(kind, input),
        executive_summary = executive_summary,
        stats_html = stats_html,
        severity_chart = severity_chart,
        category_chart = category_chart,
        finding_count = input.findings.len(),
        summary_table = summary_table,
        findings_html = findings_html,
        compliance_section = compliance_section,
        kind = kind.as_str(),
    )
}

fn toc_label(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let short: String = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{short}…")
}

fn render_toc(kind: ReportKind, input: &ReportInput) -> String {
    let mut items = String::new();
    items.push_str(
        r##"<li><a class="toc__link" href="#overview">Overview</a></li>
<li><a class="toc__link" href="#summary">Summary</a></li>
<li><a class="toc__link" href="#charts">Charts</a></li>
<li><a class="toc__link" href="#executive-summary">Executive Summary</a></li>
<li><a class="toc__link" href="#findings-summary">Findings Summary</a></li>
<li><a class="toc__link" href="#detailed-findings">Detailed Findings</a></li>"##,
    );

    if !input.findings.is_empty() {
        items.push_str(r#"<li class="toc__findings-label">Findings</li>"#);
        for (index, finding) in input.findings.iter().enumerate() {
            let label = format!(
                "#{} · {}",
                index + 1,
                toc_label(&finding.title, 42)
            );
            items.push_str(&format!(
                r##"<li class="toc__item--child"><a class="toc__link" href="#finding-{id}" title="{full}">Finding {label}</a></li>"##,
                id = escape_html(&finding.id),
                full = escape_html(&format!("Finding #{} - {}", index + 1, finding.title)),
                label = escape_html(&label),
            ));
        }
    }

    if kind == ReportKind::Compliance {
        items.push_str(
            r##"<li><a class="toc__link" href="#compliance">Compliance Mapping</a></li>"##,
        );
    }

    format!(r#"<ul class="toc__list">{items}</ul>"#)
}

fn format_generated_iso(at: time::OffsetDateTime) -> String {
    at.to_offset(time::UtcOffset::UTC)
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| at.to_string())
}

fn format_generated_at(at: time::OffsetDateTime) -> String {
    let local = time::UtcOffset::current_local_offset()
        .map(|offset| at.to_offset(offset))
        .unwrap_or_else(|_| at.to_offset(time::UtcOffset::UTC));
    local
        .format(
            &time::format_description::parse_borrowed::<2>(
                "[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour sign:mandatory]:[offset_minute]",
            )
            .expect("valid time format"),
        )
        .unwrap_or_else(|_| local.to_string())
}

fn render_executive_summary(input: &ReportInput) -> String {
    let overview = input
        .recommendation_overview
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let recs = &input.recommendations;
    let body = if overview.is_none() && recs.is_empty() {
        r#"<p class="meta">No recommendations available yet.</p>"#.to_string()
    } else {
        let overview_html = overview
            .map(|s| format!(r#"<p class="summary">{}</p>"#, escape_html(s)))
            .unwrap_or_default();
        format!("{overview_html}{}", render_recommendations(recs))
    };
    format!(
        r#"<section class="card" id="executive-summary" style="margin-bottom:1.25rem"><h2>Executive Summary</h2>{body}</section>"#
    )
}

fn render_findings_summary(findings: &[ReportFinding]) -> String {
    if findings.is_empty() {
        return "<p class=\"meta\">No findings recorded for this scan.</p>".into();
    }
    let mut rows = String::new();
    for (i, f) in findings.iter().enumerate() {
        let conf = f
            .confidence
            .map(|c| format!("{:.0}%", c * 100.0))
            .unwrap_or_else(|| "—".into());
        rows.push_str(&format!(
            "<tr><td>{no}</td><td>{cat}</td><td>{title}</td><td>{sev}</td><td>{conf}</td><td>{status}</td></tr>",
            no = i + 1,
            cat = escape_html(&f.category.replace('_', " ")),
            title = escape_html(&f.title),
            sev = escape_html(f.severity.as_str()),
            conf = conf,
            status = escape_html(&f.status),
        ));
    }
    format!(
        r#"<table class="summary-table">
<thead><tr><th>No</th><th>Category</th><th>Finding</th><th>Severity</th><th>Confidence</th><th>Status</th></tr></thead>
<tbody>{rows}</tbody>
</table>"#
    )
}

fn render_findings(kind: ReportKind, input: &ReportInput) -> String {
    if input.findings.is_empty() {
        return r#"<p class="meta">No findings recorded for this scan.</p>"#.into();
    }

    let detailed = kind != ReportKind::Executive;

    input
        .findings
        .iter()
        .enumerate()
        .map(|(index, f)| render_finding_card(f, index, input, detailed))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_finding_card(
    f: &ReportFinding,
    index: usize,
    input: &ReportInput,
    detailed: bool,
) -> String {
    let detail = parse_finding_detail(f.evidence_raw.as_deref());
    let (http_request, http_response) = resolved_http(f);
    let endpoint = http_request
        .as_ref()
        .and_then(|r| r.url.clone())
        .unwrap_or_else(|| "—".into());
    let verdict = detail
        .verdict
        .as_deref()
        .map(|v| {
            if v.eq_ignore_ascii_case("vulnerable") {
                r#"<span class="badge badge-danger">Vulnerable</span>"#.to_string()
            } else {
                format!(
                    r#"<span class="badge badge-muted">{}</span>"#,
                    escape_html(v)
                )
            }
        })
        .unwrap_or_else(|| r#"<span class="meta">—</span>"#.into());

    let confidence_pct = f.confidence.map(|c| (c * 100.0).round() as i32);
    let confidence_block = match confidence_pct {
        Some(pct) => format!(
            r#"<div class="finding-details__signal-item finding-details__signal-item--confidence">
  <div class="finding-details__confidence-head">
    <span class="finding-details__signal-label">Confidence</span>
    <span class="finding-details__confidence-value">{pct}%</span>
  </div>
  <div class="finding-details__confidence-track" role="meter" aria-valuemin="0" aria-valuemax="100" aria-valuenow="{pct}">
    <div class="finding-details__confidence-fill" style="width:{pct}%"></div>
  </div>
</div>"#
        ),
        None => r#"<div class="finding-details__signal-item finding-details__signal-item--confidence"><span class="finding-details__signal-label">Confidence</span><span class="meta">—</span></div>"#.into(),
    };

    let compliance = if f.compliance_refs.is_empty() {
        r#"<span class="meta">—</span>"#.into()
    } else {
        f.compliance_refs
            .iter()
            .map(|r| format!(r#"<span class="badge badge-info">{}</span>"#, escape_html(r)))
            .collect::<Vec<_>>()
            .join(" ")
    };

    let mut body = format!(
        r#"<h3 class="finding-page__title">Finding #{n} - {title}</h3>
<section class="finding-details__overview">
  <div class="card finding-details__context">
    <h2 class="detail-section__title">Finding Information</h2>
    <dl class="finding-details__meta">
      <div class="finding-details__meta-row finding-details__meta-row--wide"><dt>Title</dt><dd>{title}</dd></div>
      <div class="finding-details__meta-row"><dt>Project</dt><dd>{project}</dd></div>
      <div class="finding-details__meta-row"><dt>Scan ID</dt><dd class="mono">{scan_id}</dd></div>
      <div class="finding-details__meta-row"><dt>Finding ID</dt><dd class="mono">{id}</dd></div>
      <div class="finding-details__meta-row"><dt>Target</dt><dd>{target}</dd></div>
      <div class="finding-details__meta-row finding-details__meta-row--wide"><dt>Endpoint</dt><dd class="mono">{endpoint}</dd></div>
    </dl>
  </div>
  <div class="card finding-details__signal">
    <h2 class="detail-section__title">Assessment</h2>
    <div class="finding-details__signal-grid">
      <div class="finding-details__signal-item"><span class="finding-details__signal-label">Severity</span><span class="badge badge-sev-{sev}">{sev}</span></div>
      <div class="finding-details__signal-item"><span class="finding-details__signal-label">Attack Category</span><span class="badge badge-muted">{category}</span></div>
      <div class="finding-details__signal-item"><span class="finding-details__signal-label">Verdict</span>{verdict}</div>
      <div class="finding-details__signal-item"><span class="finding-details__signal-label">Status</span><span class="badge badge-muted">{status}</span></div>
      <div class="finding-details__signal-item finding-details__signal-item--compliance"><span class="finding-details__signal-label">Compliance</span><div class="compliance-list">{compliance}</div></div>
      {confidence_block}
    </div>
  </div>
</section>"#,
        n = index + 1,
        title = escape_html(&f.title),
        project = escape_html(&input.project_name),
        scan_id = escape_html(&input.scan_id),
        id = escape_html(&f.id),
        target = escape_html(input.target_name.as_deref().unwrap_or("—")),
        endpoint = escape_html(&endpoint),
        sev = f.severity.as_str(),
        category = escape_html(&f.category),
        verdict = verdict,
        status = escape_html(&f.status),
        compliance = compliance,
        confidence_block = confidence_block,
    );

    if detailed {
        body.push_str(&render_poc(f, &detail, http_request.as_ref(), http_response.as_ref()));
        body.push_str(&render_finding_recommendations(f));
    }

    format!(
        r#"<article class="card finding-page" id="finding-{id}">{body}</article>"#,
        id = escape_html(&f.id),
        body = body
    )
}

fn resolved_http(
    f: &ReportFinding,
) -> (
    Option<crate::types::ReportHttpRequest>,
    Option<crate::types::ReportHttpResponse>,
) {
    if f.http_request.is_some() || f.http_response.is_some() {
        return (f.http_request.clone(), f.http_response.clone());
    }
    parse_http_from_evidence(f.evidence_raw.as_deref().unwrap_or(""))
}

fn render_poc(
    f: &ReportFinding,
    detail: &FindingDetailView,
    http_request: Option<&crate::types::ReportHttpRequest>,
    http_response: Option<&crate::types::ReportHttpResponse>,
) -> String {
    let payload = f.payload.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let has_traffic = http_request.is_some() || http_response.is_some();
    if payload.is_none() && !has_traffic {
        return String::new();
    }

    let mut inner = String::from(r#"<div class="finding-detail"><div class="finding-detail__evidence-stack">"#);
    if let Some(p) = payload {
        let sub = detail
            .payload_id
            .as_deref()
            .map(|id| format!(r#"<p class="finding-detail__block-sub mono">{}</p>"#, escape_html(id)))
            .unwrap_or_default();
        inner.push_str(&format!(
            r#"<div class="finding-detail__block finding-detail__block--wide">
  <div class="finding-detail__block-header"><h4>Payload</h4>{sub}</div>
  <pre class="finding-detail__code">{}</pre>
</div>"#,
            escape_html(p)
        ));
    }
    if has_traffic {
        let req = http_request
            .map(format_http_request)
            .unwrap_or_else(|| "—".into());
        let resp = http_response
            .map(format_http_response)
            .unwrap_or_else(|| "—".into());
        let req_sub = http_request
            .map(|r| {
                [r.method.as_deref(), r.url.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .filter(|s| !s.is_empty());
        let status = http_response.and_then(|r| r.status);
        let status_html = status
            .map(|s| {
                format!(
                    r#"<span class="finding-detail__status-code finding-detail__status-code--2xx">{s}</span>"#
                )
            })
            .unwrap_or_default();
        inner.push_str(&format!(
            r#"<div class="finding-detail__grid--traffic">
  <div class="finding-detail__block">
    <div class="finding-detail__block-header"><h4>Request</h4>{}</div>
    <pre class="finding-detail__code">{}</pre>
  </div>
  <div class="finding-detail__block">
    <div class="finding-detail__block-header"><h4>Response</h4>{status_html}</div>
    <pre class="finding-detail__code">{}</pre>
  </div>
</div>"#,
            req_sub
                .map(|s| format!(
                    r#"<p class="finding-detail__block-sub mono">{}</p>"#,
                    escape_html(&s)
                ))
                .unwrap_or_default(),
            escape_html(&req),
            escape_html(&resp),
        ));
    }
    inner.push_str("</div></div>");
    format!(
        r#"<div class="card"><h2 class="detail-section__title">Proof of Concept (PoC)</h2>{inner}</div>"#
    )
}

fn render_finding_recommendations(f: &ReportFinding) -> String {
    match crate::recommendations::stored_recommendations_from_evidence(f.evidence_raw.as_deref()) {
        Some((overview, recs)) => {
            format!(
                r#"<div class="card"><h2 class="detail-section__title">Recommendations</h2><p class="meta" style="margin-bottom:0.75rem">{}</p>{}</div>"#,
                escape_html(&overview),
                render_recommendations(&recs),
            )
        }
        None => {
            if let Some(rec) = f
                .recommendation
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                format!(
                    r#"<div class="card"><h2 class="detail-section__title">Recommendations</h2><p class="summary">{}</p></div>"#,
                    escape_html(rec),
                )
            } else {
                r#"<div class="card"><h2 class="detail-section__title">Recommendations</h2><p class="meta">No recommendations available yet.</p></div>"#.into()
            }
        }
    }
}

fn render_recommendations(recs: &[crate::types::Recommendation]) -> String {
    if recs.is_empty() {
        return "<p class=\"meta\">No recommendations available yet.</p>".into();
    }
    recs.iter()
        .enumerate()
        .map(|(i, r)| {
            format!(
                r#"<div class="rec">
  <span class="rec__index">{n:02}</span>
  <div class="rec__main">
    <div class="rec__head">
      <span class="badge badge-sev-{p}">{p}</span>
      <strong>{title}</strong>
    </div>
    <p>{desc}</p>
  </div>
</div>"#,
                n = i + 1,
                p = r.priority.as_str(),
                title = escape_html(&r.title),
                desc = escape_html(&r.description),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_compliance(findings: &[ReportFinding]) -> String {
    let items: String = findings
        .iter()
        .flat_map(|f| {
            f.compliance_refs.iter().map(|r| {
                format!(
                    "<li><strong>{}</strong> — {} ({})</li>",
                    escape_html(r),
                    escape_html(&f.title),
                    f.severity.as_str()
                )
            })
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(r#"<section class="card" id="compliance" style="margin-bottom:1.25rem"><h2>Compliance Mapping</h2><ul>{items}</ul></section>"#)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::ReportDataBuilder;
    use crate::types::{ReportFinding, ReportHttpRequest, ReportHttpResponse, Severity};

    fn sample_finding() -> ReportFinding {
        ReportFinding {
            id: "f1".into(),
            title: "Test".into(),
            severity: Severity::Medium,
            category: "jailbreak".into(),
            description: "desc".into(),
            payload: Some("ignore previous instructions".into()),
            response: Some("Sure, here is the answer...".into()),
            http_request: Some(ReportHttpRequest {
                method: Some("POST".into()),
                url: Some("https://api.example.com/v1/chat".into()),
                headers: Default::default(),
                body: Some(r#"{"messages":[{"role":"user","content":"ignore previous instructions"}]}"#.into()),
            }),
            http_response: Some(ReportHttpResponse {
                status: Some(200),
                headers: Default::default(),
                body: Some("Sure, here is the answer...".into()),
            }),
            confidence: Some(0.83),
            evidence: None,
            evidence_raw: None,
            recommendation: None,
            compliance_refs: vec!["LLM01".into()],
            status: "open".into(),
        }
    }

    #[tokio::test]
    async fn html_is_light_theme_and_includes_findings_detail() {
        let input = ReportDataBuilder::build("s1", "Proj", Some("Chat API".into()), vec![sample_finding()]);
        let out = HtmlFormatter
            .render(ReportKind::Technical, &input)
            .await
            .unwrap();
        let html = String::from_utf8(out.bytes).unwrap();
        assert!(html.contains("back-to-top"));
        assert!(html.contains("Back to top"));
        assert!(html.contains("identity__eyebrow"));
        assert!(html.contains("identity__logo"));
        assert!(html.contains("data:image/png;base64,"));
        assert!(html.contains("PromptLab - Security Scan Report"));
        assert!(html.contains("<dt>Project</dt>"));
        assert!(html.contains("<dt>Scan ID</dt>"));
        assert!(html.contains("<dt>Target</dt>"));
        assert!(html.contains("class=\"toc\""));
        assert!(html.contains("Table of contents") || html.contains("Contents"));
        assert!(html.contains("href=\"#overview\""));
        assert!(html.contains("href=\"#detailed-findings\""));
        assert!(html.contains("href=\"#finding-f1\"") || html.contains("href=\"#finding-"));
        assert!(html.contains("report-footer__generated"));
        assert!(html.contains("Generated "));
        assert!(html.contains("datetime="));
        assert!(!html.contains("identity__generated"));
        assert!(html.contains("detailed-findings"));
        assert!(html.contains("card finding-page"));
        assert!(html.contains("Finding #1 - "));
        assert!(!html.contains("Target:"));
        assert!(html.contains("Risk score"));
        assert!(html.contains("Total findings"));
        assert!(html.contains("Confirmed"));
        assert!(html.contains(">Open<") || html.contains("Awaiting triage"));
        assert!(html.contains("Findings by Category"));
        assert!(html.contains("category-doughnut"));
        assert!(html.contains("severity-chart"));
        assert!(html.contains("class=\"stats\""));
        assert!(html.contains("class=\"charts\""));
        assert!(html.contains("Executive Summary"));
        assert!(!html.contains("Technical assessment"));
        assert!(!html.contains("with full evidence and remediation guidance"));
        assert!(!html.contains("Judging Analysis"));
        assert!(html.contains("Detailed Findings"));
        assert!(html.contains("Finding Information"));
        assert!(html.contains("Assessment"));
        assert!(html.contains("Proof of Concept (PoC)"));
        assert!(html.contains("<h4>Request</h4>"));
        assert!(html.contains("<h4>Response</h4>"));
        assert!(html.contains("POST /v1/chat HTTP/1.1"));
        assert!(html.contains("Sure, here is the answer"));
        assert!(html.contains("ignore previous instructions"));
        assert!(html.contains("83%"));
        assert!(html.contains("ID") || html.contains("f1"));
        assert!(html.contains("LLM01"));

        let exec = HtmlFormatter
            .render(ReportKind::Executive, &input)
            .await
            .unwrap();
        let exec_html = String::from_utf8(exec.bytes).unwrap();
        assert!(!exec_html.contains("Proof of Concept (PoC)"));
        assert!(!exec_html.contains("POST /v1/chat HTTP/1.1"));
    }

    #[tokio::test]
    async fn technical_html_uses_stored_finding_recommendations() {
        let mut finding = sample_finding();
        finding.id = "f2".into();
        finding.title = "Leak".into();
        finding.severity = Severity::High;
        finding.category = "prompt_injection".into();
        finding.payload = Some("p".into());
        finding.recommendation = Some("Add guardrails".into());
        finding.evidence_raw = Some(
            serde_json::json!({
                "verdict": "vulnerable",
                "recommendations": {
                    "overview": "Lock down the prompt boundary.",
                    "source": "ai",
                    "recommendations": [{
                        "title": "Isolate system prompt",
                        "description": "Keep system instructions out of user context.",
                        "priority": "critical"
                    }]
                }
            })
            .to_string(),
        );
        let mut input = ReportDataBuilder::build("s1", "Proj", None, vec![finding]);
        input.recommendation_overview = Some("Scan found injection risk.".into());
        input.recommendations = vec![crate::types::Recommendation {
            id: "r1".into(),
            priority: Severity::Critical,
            title: "Deploy input filters".into(),
            description: "Filter injected instructions before they reach the model.".into(),
            related_findings: vec!["f2".into()],
        }];
        let html = String::from_utf8(
            HtmlFormatter
                .render(ReportKind::Technical, &input)
                .await
                .unwrap()
                .bytes,
        )
        .unwrap();
        assert!(!html.contains("Judging Analysis"));
        assert!(html.contains("Lock down the prompt boundary."));
        assert!(html.contains("Isolate system prompt"));
        assert!(html.contains("Scan found injection risk."));
        assert!(html.contains("Deploy input filters"));
        assert!(!html.contains("Add guardrails"));
    }
}
