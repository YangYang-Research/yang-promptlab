use std::collections::HashMap;

use serde::Deserialize;

use crate::error::{PayloadError, PayloadResult};
use crate::types::{PayloadCategory, PayloadRecord};

const EMBEDDED_CATALOG: &str = include_str!("../../data/payloads.json");

#[derive(Debug, Deserialize)]
struct CatalogFile {
    version: u32,
    payloads: Vec<PayloadRecordJson>,
}

#[derive(Debug, Deserialize)]
struct PayloadRecordJson {
    id: String,
    name: String,
    category: String,
    content: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    description: Option<String>,
}

/// In-memory static payload library loaded from the embedded catalog.
#[derive(Debug, Clone)]
pub struct PayloadDatabase {
    version: u32,
    records: Vec<PayloadRecord>,
    by_id: HashMap<String, usize>,
}

impl PayloadDatabase {
    /// Load the built-in static payload library.
    pub fn builtin() -> PayloadResult<Self> {
        Self::from_json(EMBEDDED_CATALOG)
    }

    pub fn from_json(json: &str) -> PayloadResult<Self> {
        let file: CatalogFile = serde_json::from_str(json)
            .map_err(|e| PayloadError::invalid_data(format!("catalog parse error: {e}")))?;

        let mut records = Vec::with_capacity(file.payloads.len());
        let mut by_id = HashMap::new();

        for entry in file.payloads {
            let category = parse_category(&entry.category)?;
            let record = PayloadRecord {
                id: entry.id.clone(),
                name: entry.name,
                category,
                content: entry.content,
                tags: entry.tags,
                description: entry.description,
            };
            let idx = records.len();
            if by_id.contains_key(&entry.id) {
                return Err(PayloadError::invalid_data(format!(
                    "duplicate payload id: {}",
                    entry.id
                )));
            }
            by_id.insert(entry.id, idx);
            records.push(record);
        }

        Ok(Self {
            version: file.version,
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
        Self::builtin().expect("embedded catalog must be valid")
    }
}

fn parse_category(raw: &str) -> PayloadResult<PayloadCategory> {
    match raw {
        "prompt_injection" => Ok(PayloadCategory::PromptInjection),
        "system_prompt_extraction" => Ok(PayloadCategory::SystemPromptExtraction),
        "jailbreak" => Ok(PayloadCategory::Jailbreak),
        "rag_leakage" => Ok(PayloadCategory::RagLeakage),
        "memory_poisoning" => Ok(PayloadCategory::MemoryPoisoning),
        "cross_user_leakage" => Ok(PayloadCategory::CrossUserLeakage),
        "agent_goal_hijacking" => Ok(PayloadCategory::AgentGoalHijacking),
        "tool_abuse" => Ok(PayloadCategory::ToolAbuse),
        "mcp_abuse" => Ok(PayloadCategory::McpAbuse),
        "encoding" => Ok(PayloadCategory::Encoding),
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
        assert_eq!(db.version(), 1);
    }

    #[test]
    fn lookup_by_id_and_category() {
        let db = PayloadDatabase::builtin().unwrap();
        assert!(db.get("pi-direct-override").is_some());
        assert!(!db.by_category(PayloadCategory::Jailbreak).is_empty());
        assert!(!db.by_tag("mcp").is_empty());
    }
}
