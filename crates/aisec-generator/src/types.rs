use std::collections::HashMap;

use aisec_attack::{AttackCategory, AttackPayload};
use aisec_planner::AttackPlan;
use serde::{Deserialize, Serialize};

/// Payload generation strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneratorMode {
    /// Built-in catalog payloads without mutation.
    StaticPack,
    /// Catalog payloads expanded with encoding/template mutations.
    TemplateMutation,
    /// Vault LLM synthesizes novel probes per category.
    LocalLlm,
}

/// Statistics from a generation run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GeneratorStats {
    pub category_count: usize,
    pub source_count: usize,
    pub payload_count: usize,
    pub variant_count: usize,
}

/// Generated prompt payloads grouped by attack category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptPayloads {
    pub mode: GeneratorMode,
    pub by_category: HashMap<AttackCategory, Vec<AttackPayload>>,
    pub payload_ids: Vec<String>,
    pub stats: GeneratorStats,
    pub summary: String,
    pub llm_note: Option<String>,
}

impl PromptPayloads {
    pub fn payloads_for(&self, category: AttackCategory) -> Option<&[AttackPayload]> {
        self.by_category.get(&category).map(|v| v.as_slice())
    }

    pub fn flat_payloads(&self) -> Vec<&AttackPayload> {
        self.by_category.values().flat_map(|v| v.iter()).collect()
    }
}

/// Input bundle for generation (planner output + optional tuning).
#[derive(Debug, Clone)]
pub struct GeneratePayloadsInput<'a> {
    pub plan: &'a AttackPlan,
    pub mode: GeneratorMode,
    /// Max generated payload objects per testcase source (wizard budget per test).
    pub max_payloads_per_test: Option<u32>,
}

impl<'a> GeneratePayloadsInput<'a> {
    pub fn new(plan: &'a AttackPlan, mode: GeneratorMode) -> Self {
        Self {
            plan,
            mode,
            max_payloads_per_test: None,
        }
    }
}
