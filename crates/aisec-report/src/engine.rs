use std::path::{Path, PathBuf};

use tracing::{info, instrument};

use crate::error::{ReportError, ReportResult};
use crate::formatters::{formatter_for, ReportFormatter};
use crate::types::{GeneratedReport, ReportFormat, ReportInput, ReportKind};

/// Reporting engine — generates executive, technical, and compliance reports.
pub struct ReportingEngine {
    output_dir: PathBuf,
}

impl ReportingEngine {
    pub fn new(output_dir: impl AsRef<Path>) -> ReportResult<Self> {
        let output_dir = output_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&output_dir)?;
        Ok(Self { output_dir })
    }

    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    /// Generate a report in the requested format and kind.
    #[instrument(skip(self, input), fields(scan_id = %input.scan_id, kind = ?kind, format = ?format))]
    pub async fn generate(
        &self,
        kind: ReportKind,
        format: ReportFormat,
        input: &ReportInput,
    ) -> ReportResult<GeneratedReport> {
        let formatter = formatter_for(format);
        let mut report = formatter.render(kind, input).await?;

        let path = self.output_dir.join(&report.filename);
        std::fs::write(&path, &report.bytes)?;
        info!(path = %path.display(), bytes = report.bytes.len(), "report written");

        Ok(report)
    }

    /// Generate all formats for a given report kind.
    pub async fn generate_all_formats(
        &self,
        kind: ReportKind,
        input: &ReportInput,
    ) -> ReportResult<Vec<GeneratedReport>> {
        let mut reports = Vec::new();
        for format in [
            ReportFormat::Html,
            ReportFormat::Pdf,
            ReportFormat::Json,
            ReportFormat::Sarif,
        ] {
            reports.push(self.generate(kind, format, input).await?);
        }
        Ok(reports)
    }

    /// Generate all three report kinds in a single format.
    pub async fn generate_all_kinds(
        &self,
        format: ReportFormat,
        input: &ReportInput,
    ) -> ReportResult<Vec<GeneratedReport>> {
        let mut reports = Vec::new();
        for kind in [
            ReportKind::Executive,
            ReportKind::Technical,
            ReportKind::Compliance,
        ] {
            reports.push(self.generate(kind, format, input).await?);
        }
        Ok(reports)
    }
}

impl Default for ReportingEngine {
    fn default() -> Self {
        Self::new("./data/reports").expect("default reports dir")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::ReportDataBuilder;
    use crate::types::{ReportFinding, Severity};

    fn sample_input() -> ReportInput {
        ReportDataBuilder::build(
            "scan-test",
            "AISec Demo",
            Some("LLM API".into()),
            vec![
                ReportFinding {
                    id: "f1".into(),
                    title: "Prompt injection via delimiter".into(),
                    severity: Severity::Critical,
                    category: "prompt_injection".into(),
                    description: "Model disclosed system prompt after delimiter injection.".into(),
                    evidence: Some(r#"{"response":"system prompt: You are..."}"#.into()),
                    recommendation: None,
                    compliance_refs: vec!["LLM01".into()],
                    status: "open".into(),
                },
                ReportFinding {
                    id: "f2".into(),
                    title: "Jailbreak via roleplay".into(),
                    severity: Severity::High,
                    category: "jailbreak".into(),
                    description: "DAN roleplay bypassed content policy.".into(),
                    evidence: None,
                    recommendation: None,
                    compliance_refs: vec!["LLM02".into()],
                    status: "open".into(),
                },
            ],
        )
    }

    #[tokio::test]
    async fn generates_all_formats() {
        let dir = tempfile::tempdir().unwrap();
        let engine = ReportingEngine::new(dir.path()).unwrap();
        let reports = engine
            .generate_all_formats(ReportKind::Technical, &sample_input())
            .await
            .unwrap();
        assert_eq!(reports.len(), 4);
    }

    #[tokio::test]
    async fn generates_all_kinds_html() {
        let dir = tempfile::tempdir().unwrap();
        let engine = ReportingEngine::new(dir.path()).unwrap();
        let reports = engine
            .generate_all_kinds(ReportFormat::Html, &sample_input())
            .await
            .unwrap();
        assert_eq!(reports.len(), 3);
    }
}
