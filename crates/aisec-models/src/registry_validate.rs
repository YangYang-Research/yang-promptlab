//! Built-in model registry validation (GGUF-first schema).

use serde::{Deserialize, Serialize};

use crate::builtin_catalog::BuiltinRegistryEntry;

const SUPPORTED_ENGINE: &str = "llama.cpp";
const SUPPORTED_FORMAT: &str = "gguf";

/// Single validation issue for a registry row.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryValidationIssue {
    pub id: String,
    pub field: String,
    pub message: String,
}

/// Startup validation report for `resources/models.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryValidationReport {
    pub total: usize,
    pub valid: usize,
    pub invalid: usize,
    pub issues: Vec<RegistryValidationIssue>,
    pub valid_ids: Vec<String>,
    pub invalid_ids: Vec<String>,
}

impl RegistryValidationReport {
    pub fn is_healthy(&self) -> bool {
        self.invalid == 0 && self.total > 0
    }
}

/// Validate registry entries. Returns `(valid_entries, report)`.
pub fn validate_registry(entries: &[BuiltinRegistryEntry]) -> (Vec<BuiltinRegistryEntry>, RegistryValidationReport) {
    let mut report = RegistryValidationReport {
        total: entries.len(),
        ..Default::default()
    };
    let mut seen_ids = std::collections::HashSet::new();
    let mut valid = Vec::new();

    for entry in entries {
        let mut entry_issues = Vec::new();

        if entry.id.trim().is_empty() {
            entry_issues.push(RegistryValidationIssue {
                id: entry.id.clone(),
                field: "id".into(),
                message: "id must not be empty".into(),
            });
        } else if !seen_ids.insert(entry.id.clone()) {
            entry_issues.push(RegistryValidationIssue {
                id: entry.id.clone(),
                field: "id".into(),
                message: "duplicate id".into(),
            });
        }

        if entry.name.trim().is_empty() {
            entry_issues.push(RegistryValidationIssue {
                id: entry.id.clone(),
                field: "name".into(),
                message: "name must not be empty".into(),
            });
        }

        let engine = entry.engine.trim().to_ascii_lowercase();
        if engine != SUPPORTED_ENGINE {
            entry_issues.push(RegistryValidationIssue {
                id: entry.id.clone(),
                field: "engine".into(),
                message: format!("unsupported engine '{engine}'; expected {SUPPORTED_ENGINE}"),
            });
        }

        let format = entry.format.trim().to_ascii_lowercase();
        if format != SUPPORTED_FORMAT {
            entry_issues.push(RegistryValidationIssue {
                id: entry.id.clone(),
                field: "format".into(),
                message: format!("unsupported format '{format}'; expected {SUPPORTED_FORMAT}"),
            });
        }

        if entry.download_url.trim().is_empty() {
            entry_issues.push(RegistryValidationIssue {
                id: entry.id.clone(),
                field: "download_url".into(),
                message: "missing download_url".into(),
            });
        } else if !entry.download_url.starts_with("https://") {
            entry_issues.push(RegistryValidationIssue {
                id: entry.id.clone(),
                field: "download_url".into(),
                message: "download_url must be an https URL".into(),
            });
        }

        if let Some(sha) = entry.sha256.as_deref() {
            if !sha.trim().is_empty() && !is_valid_sha256(sha.trim()) {
                entry_issues.push(RegistryValidationIssue {
                    id: entry.id.clone(),
                    field: "sha256".into(),
                    message: "invalid SHA256 (expected 64 hex characters)".into(),
                });
            }
        }

        if entry_issues.is_empty() {
            report.valid += 1;
            report.valid_ids.push(entry.id.clone());
            valid.push(entry.clone());
        } else {
            report.invalid += 1;
            report.invalid_ids.push(entry.id.clone());
            report.issues.extend(entry_issues);
        }
    }

    (valid, report)
}

fn is_valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin_catalog::BuiltinRegistryEntry;

    fn sample() -> BuiltinRegistryEntry {
        BuiltinRegistryEntry {
            id: "test".into(),
            name: "Test".into(),
            purpose: "judge".into(),
            recommended: false,
            engine: "llama.cpp".into(),
            format: "gguf".into(),
            size: Some("4GB".into()),
            sha256: None,
            download_url: "https://example.com/model.gguf".into(),
        }
    }

    #[test]
    fn accepts_valid_entry() {
        let (valid, report) = validate_registry(&[sample()]);
        assert_eq!(valid.len(), 1);
        assert_eq!(report.invalid, 0);
    }

    #[test]
    fn rejects_duplicate_ids() {
        let a = sample();
        let b = sample();
        let (_, report) = validate_registry(&[a, b]);
        assert_eq!(report.invalid, 1);
        assert!(report.issues.iter().any(|i| i.field == "id"));
    }

    #[test]
    fn rejects_bad_sha256() {
        let mut entry = sample();
        entry.sha256 = Some("not-a-hash".into());
        let (_, report) = validate_registry(&[entry]);
        assert_eq!(report.invalid, 1);
    }
}
