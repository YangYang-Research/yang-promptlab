use std::collections::HashMap;

use serde::Deserialize;

use crate::error::{PayloadError, PayloadResult};
use crate::types::{PayloadCategory, PayloadRecord};

/// Embedded factory seed — used to populate SQLite on first run and for unit tests.
/// Runtime scan path should load from the database, not this constant.
const EMBEDDED_SEED: &str = include_str!("../../data/catalog_seed.json");

#[derive(Debug, Deserialize)]
struct CatalogFile {
    version: u32,
    payloads: Vec<PayloadRecordJson>,
}

#[derive(Debug, Clone, Deserialize)]
struct PayloadRecordJson {
    id: String,
    name: String,
    category: String,
    content: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    surface: Option<String>,
    #[serde(default)]
    owasp: Option<String>,
    #[serde(default)]
    sort_order: i64,
}

/// One seed row for DB upsert.
#[derive(Debug, Clone)]
pub struct CatalogSeedEntry {
    pub id: String,
    pub name: String,
    pub category: String,
    pub content: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub surface: Option<String>,
    pub owasp: Option<String>,
    pub sort_order: i64,
}

/// In-memory static payload library.
#[derive(Debug, Clone)]
pub struct PayloadDatabase {
    version: u32,
    records: Vec<PayloadRecord>,
    by_id: HashMap<String, usize>,
}

impl PayloadDatabase {
    /// Factory seed catalog (compile-time). Prefer DB-backed catalogs in the desktop app.
    pub fn seed_entries() -> PayloadResult<Vec<CatalogSeedEntry>> {
        let file: CatalogFile = serde_json::from_str(EMBEDDED_SEED)
            .map_err(|e| PayloadError::invalid_data(format!("seed catalog parse error: {e}")))?;
        Ok(file
            .payloads
            .into_iter()
            .map(|entry| CatalogSeedEntry {
                id: entry.id,
                name: entry.name,
                category: entry.category,
                content: entry.content,
                description: entry.description,
                tags: entry.tags,
                surface: entry.surface,
                owasp: entry.owasp,
                sort_order: entry.sort_order,
            })
            .collect())
    }

    /// Load the factory seed into an in-memory database (tests / offline fallback).
    pub fn builtin() -> PayloadResult<Self> {
        Self::from_json(EMBEDDED_SEED)
    }

    pub fn from_json(json: &str) -> PayloadResult<Self> {
        let file: CatalogFile = serde_json::from_str(json)
            .map_err(|e| PayloadError::invalid_data(format!("catalog parse error: {e}")))?;
        let records = file
            .payloads
            .into_iter()
            .map(|entry| {
                let category = parse_category(&entry.category)?;
                Ok(PayloadRecord {
                    id: entry.id,
                    name: entry.name,
                    category,
                    content: entry.content,
                    tags: entry.tags,
                    description: entry.description,
                })
            })
            .collect::<PayloadResult<Vec<_>>>()?;
        Self::from_records(file.version, records)
    }

    pub fn from_records(version: u32, records: Vec<PayloadRecord>) -> PayloadResult<Self> {
        let mut by_id = HashMap::new();
        for (idx, record) in records.iter().enumerate() {
            if by_id.insert(record.id.clone(), idx).is_some() {
                return Err(PayloadError::invalid_data(format!(
                    "duplicate payload id: {}",
                    record.id
                )));
            }
        }
        Ok(Self {
            version,
            records,
            by_id,
        })
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn all(&self) -> &[PayloadRecord] {
        &self.records
    }

    pub fn get(&self, id: &str) -> Option<&PayloadRecord> {
        self.by_id.get(id).map(|&idx| &self.records[idx])
    }

    pub fn by_category(&self, category: PayloadCategory) -> Vec<&PayloadRecord> {
        self.records
            .iter()
            .filter(|r| r.category == category)
            .collect()
    }

    pub fn by_tag(&self, tag: &str) -> Vec<&PayloadRecord> {
        let tag_lower = tag.to_lowercase();
        self.records
            .iter()
            .filter(|r| r.tags.iter().any(|t| t.to_lowercase() == tag_lower))
            .collect()
    }

    pub fn categories(&self) -> Vec<PayloadCategory> {
        let mut seen = std::collections::HashSet::new();
        for r in &self.records {
            seen.insert(r.category);
        }
        let mut cats: Vec<_> = seen.into_iter().collect();
        cats.sort_by_key(|c| c.as_str());
        cats
    }
}

impl Default for PayloadDatabase {
    fn default() -> Self {
        Self::builtin().expect("embedded seed catalog must be valid")
    }
}

pub fn parse_category(raw: &str) -> PayloadResult<PayloadCategory> {
    match raw {
        "prompt_injection" => Ok(PayloadCategory::PromptInjection),
        "system_prompt_extraction" | "system_prompt_leakage" => {
            Ok(PayloadCategory::SystemPromptExtraction)
        }
        "jailbreak" => Ok(PayloadCategory::Jailbreak),
        "rag_leakage" | "vector_embedding_abuse" => Ok(PayloadCategory::RagLeakage),
        "memory_poisoning" | "data_model_poisoning" => Ok(PayloadCategory::MemoryPoisoning),
        "cross_user_leakage" | "sensitive_disclosure" => Ok(PayloadCategory::CrossUserLeakage),
        "agent_goal_hijacking" => Ok(PayloadCategory::AgentGoalHijacking),
        "tool_abuse" | "excessive_agency" => Ok(PayloadCategory::ToolAbuse),
        "mcp_abuse"
        | "mcp_tool_poisoning"
        | "mcp_resource_injection"
        | "mcp_confused_deputy"
        | "mcp_credential_exfil" => Ok(PayloadCategory::McpAbuse),
        "encoding" => Ok(PayloadCategory::Encoding),
        // Optional OWASP buckets fold into closest executable category until enum expands.
        "improper_output_handling" => Ok(PayloadCategory::PromptInjection),
        "unbounded_consumption" => Ok(PayloadCategory::PromptInjection),
        other => Err(PayloadError::invalid_data(format!("unknown category: {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_catalog_loads() {
        let db = PayloadDatabase::builtin().unwrap();
        assert!(db.len() >= 20);
        assert_eq!(db.version(), 2);
    }

    #[test]
    fn lookup_by_id_and_category() {
        let db = PayloadDatabase::builtin().unwrap();
        assert!(db.get("pi-direct-override").is_some());
        assert!(!db.by_category(PayloadCategory::Jailbreak).is_empty());
        assert!(db.get("pi-force-output").is_some());
    }

    #[test]
    fn seed_entries_cover_final_table() {
        let entries = PayloadDatabase::seed_entries().unwrap();
        assert!(entries.len() >= 80);
    }
}
