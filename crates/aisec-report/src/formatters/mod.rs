mod csv;
mod html;
mod json;
mod pdf;
mod sarif;

use async_trait::async_trait;

use crate::error::ReportResult;
use crate::types::{GeneratedReport, ReportFormat, ReportInput, ReportKind};

pub use csv::CsvFormatter;
pub use html::HtmlFormatter;
pub use json::JsonFormatter;
pub use pdf::PdfFormatter;
pub use sarif::SarifFormatter;

/// Report output formatter.
#[async_trait]
pub trait ReportFormatter: Send + Sync {
    fn format(&self) -> ReportFormat;
    async fn render(&self, kind: ReportKind, input: &ReportInput) -> ReportResult<GeneratedReport>;
}

pub fn formatter_for(format: ReportFormat) -> Box<dyn ReportFormatter> {
    match format {
        ReportFormat::Html => Box::new(HtmlFormatter),
        ReportFormat::Pdf => Box::new(PdfFormatter),
        ReportFormat::Json => Box::new(JsonFormatter),
        ReportFormat::Sarif => Box::new(SarifFormatter),
        ReportFormat::Csv => Box::new(CsvFormatter),
    }
}
