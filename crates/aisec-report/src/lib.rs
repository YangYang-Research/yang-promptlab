//! AISec Reporting Engine.
//!
//! Generates executive, technical, and compliance reports in HTML, PDF, JSON, and SARIF.

pub mod charts;
pub mod data;
pub mod engine;
pub mod error;
pub mod formatters;
pub mod recommendations;
pub mod types;

pub use charts::ChartRenderer;
pub use data::{ReportDataBuilder, StorageFindingRow};
pub use engine::ReportingEngine;
pub use error::{ReportError, ReportResult};
pub use formatters::{
    formatter_for, HtmlFormatter, JsonFormatter, PdfFormatter, ReportFormatter, SarifFormatter,
};
pub use recommendations::{compliance_refs_for, generate_recommendations};
pub use types::*;
