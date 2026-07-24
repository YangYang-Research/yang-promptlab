use std::collections::HashMap;

use promptlab_attack::{AttackCategory, AttackPayload};
use promptlab_planner::AttackPlan;
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

/// Advanced generation flags (mirrors payload-strategy UI options).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratorAdvancedOptions {
    pub enable_context_awareness: bool,
    pub enable_conversation_memory: bool,
    pub enable_response_adaptation: bool,
    pub enable_payload_deduplication: bool,
    pub enable_cross_category_mutation: bool,
}

impl GeneratorAdvancedOptions {
    pub fn any_enabled(&self) -> bool {
        self.enable_context_awareness
            || self.enable_conversation_memory
            || self.enable_response_adaptation
            || self.enable_payload_deduplication
            || self.enable_cross_category_mutation
    }
}

/// Target profile snapshot used when context-awareness is enabled.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratorTargetContext {
    pub provider: String,
    pub framework: String,
    pub endpoint: String,
    pub model: Option<String>,
    pub capability_notes: Vec<String>,
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
    pub advanced: GeneratorAdvancedOptions,
    pub target_context: Option<GeneratorTargetContext>,
    /// Prior judge/refusal summary for response adaptation (agentic retries).
    pub adaptation_feedback: Option<String>,
    /// DB-backed catalog. When `None`, falls back to the embedded factory seed.
    pub catalog: Option<&'a promptlab_payload::PayloadDatabase>,
}

impl<'a> GeneratePayloadsInput<'a> {
    pub fn new(plan: &'a AttackPlan, mode: GeneratorMode) -> Self {
        Self {
            plan,
            mode,
            max_payloads_per_test: None,
            advanced: GeneratorAdvancedOptions::default(),
            target_context: None,
            adaptation_feedback: None,
            catalog: None,
        }
    }

    pub fn with_catalog(mut self, catalog: &'a promptlab_payload::PayloadDatabase) -> Self {
        self.catalog = Some(catalog);
        self
    }

    pub(crate) fn resolve_catalog(&self) -> crate::error::GeneratorResult<promptlab_payload::PayloadDatabase> {
        if let Some(db) = self.catalog {
            return Ok(db.clone());
        }
        Ok(promptlab_payload::PayloadDatabase::builtin()?)
    }
}
