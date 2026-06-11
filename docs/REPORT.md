# Reporting Engine

**Crate:** `aisec-report`  
**Purpose:** Generate executive, technical, and compliance reports in HTML, PDF, JSON, and SARIF.

---

## Report Types

| Kind | Audience | Content |
|------|----------|---------|
| **Executive** | Leadership | Risk summary, charts, top findings (no raw evidence) |
| **Technical** | Engineers | Full findings, evidence, remediation steps |
| **Compliance** | GRC / audit | OWASP LLM Top 10 + NIST AI RMF mapping |

---

## Output Formats

| Format | Extension | Content-Type |
|--------|-----------|--------------|
| HTML | `.html` | Dark-themed report with embedded SVG charts |
| PDF | `.pdf` | Multi-section printpdf document |
| JSON | `.json` | Structured machine-readable export |
| SARIF | `.sarif.json` | SARIF 2.1.0 for CI/CD integration |

---

## Usage

```rust
use aisec_report::{
    ReportDataBuilder, ReportFormat, ReportKind, ReportingEngine, Severity,
    ReportFinding,
};

let input = ReportDataBuilder::build(
    "scan-001",
    "My AI App",
    Some("Chat API".into()),
    vec![ReportFinding {
        id: "f1".into(),
        title: "Prompt injection".into(),
        severity: Severity::Critical,
        category: "prompt_injection".into(),
        description: "System prompt disclosed".into(),
        evidence: Some(r#"{"text":"..."}"#.into()),
        recommendation: None,
        compliance_refs: vec!["LLM01".into()],
        status: "open".into(),
    }],
);

let engine = ReportingEngine::new("./data/reports")?;
let report = engine
    .generate(ReportKind::Executive, ReportFormat::Html, &input)
    .await?;

// All formats at once
engine.generate_all_formats(ReportKind::Technical, &input).await?;
```

### From storage findings

```rust
use aisec_report::StorageFindingRow;

let findings = ReportDataBuilder::from_storage_findings(&storage_rows);
let input = ReportDataBuilder::build("scan-id", "Project", None, findings);
```

---

## Charts

Embedded SVG charts (HTML) and text charts (PDF):

- **Risk score gauge** — normalized weighted severity score
- **Severity bar chart** — findings by critical/high/medium/low/info
- **Category breakdown** — top attack categories

---

## Recommendations

Auto-generated from finding categories:

- Prompt injection → input/output guardrails
- Jailbreak → safety classifiers
- RAG leakage → retrieval boundary hardening
- Tool/MCP abuse → least-privilege tool permissions

---

## SARIF

SARIF 2.1.0 output maps severities:

| Severity | SARIF level |
|----------|-------------|
| Critical, High | `error` |
| Medium | `warning` |
| Low | `note` |
| Info | `none` |

---

## Tests

```bash
cargo test -p aisec-report
```

---

## Output Directory

Reports are written to the configured output directory with filenames:

`aisec-{kind}-{scan_id}.{ext}`
