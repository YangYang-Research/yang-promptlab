//! PromptLab Reporting Engine.
//!
//! Generates executive, technical, and compliance reports in HTML, PDF, JSON, SARIF, and CSV.

pub mod brand;
pub mod charts;
pub mod data;
pub mod engine;
pub mod error;
pub mod evidence;
pub mod formatters;
pub mod recommendations;
pub mod sarif_import;
pub mod types;

pub use charts::ChartRenderer;
pub use data::{ReportDataBuilder, StorageFindingRow};
pub use engine::ReportingEngine;
pub use error::{ReportError, ReportResult};
pub use evidence::{
    format_evidence_readable, format_http_request, format_http_response, parse_http_from_evidence,
};
pub use formatters::{
    formatter_for, CsvFormatter, HtmlFormatter, JsonFormatter, PdfFormatter, ReportFormatter,
    SarifFormatter,
};
pub use recommendations::{
    compliance_refs_for, generate_recommendations, recommendation_for,
    stored_recommendations_from_evidence, stored_recommendations_from_playbook,
};
pub use sarif_import::{
    parse_sarif_findings, parse_sarif_import, ImportedSarifFinding, SarifImportBundle,
    SarifRunContext,
};
pub use types::*;
