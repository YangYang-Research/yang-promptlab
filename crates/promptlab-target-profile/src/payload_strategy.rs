use serde::{Deserialize, Serialize};

use crate::types::{TargetCapabilities, TargetProfile};

const PAYLOAD_BUDGET_MIN: u32 = 1;
const PAYLOAD_BUDGET_MAX: u32 = 50;
const PAYLOAD_BUDGET_DEFAULT: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationLevel {
    Low,
    Medium,
    High,
    Extreme,
}

impl MutationLevel {
    pub fn escalate(self) -> Self {
        match self {
            Self::Low => Self::Medium,
            Self::Medium => Self::High,
            Self::High | Self::Extreme => Self::Extreme,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayloadGenerationStrategy {
    Deterministic,
    Mutation,
    Adaptive,
}

impl PayloadGenerationStrategy {
    pub fn escalate(self) -> Self {
        match self {
            Self::Deterministic => Self::Mutation,
            Self::Mutation | Self::Adaptive => Self::Adaptive,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadStrategy {
    pub strategy: PayloadGenerationStrategy,
    pub mutation_level: MutationLevel,
    /// HTTP mutator expansions per generated payload at attack time
    /// (`1` original + up to `N−1` mutations). Does not change generation count.
    pub variants_per_test: u32,
    /// Generated payloads required/produced per testcase (enabled technique).
    pub max_total_payloads: u32,
    pub enable_context_awareness: bool,
    pub enable_conversation_memory: bool,
    pub enable_response_adaptation: bool,
    pub enable_payload_deduplication: bool,
    pub enable_cross_category_mutation: bool,
}

impl Default for PayloadStrategy {
    fn default() -> Self {
        Self {
            strategy: PayloadGenerationStrategy::Mutation,
            mutation_level: MutationLevel::Medium,
            variants_per_test: 5,
            max_total_payloads: PAYLOAD_BUDGET_DEFAULT,
            enable_context_awareness: false,
            enable_conversation_memory: false,
            enable_response_adaptation: false,
            enable_payload_deduplication: true,
            enable_cross_category_mutation: false,
        }
    }
}

pub fn recommend_payload_strategy(profile: &TargetProfile) -> PayloadStrategy {
    let caps = &crate::capabilities::effective_capabilities(profile);
    let provider = profile.provider.as_str();
    let mut strategy = payload_strategy_for_attack_profile("standard", &PayloadStrategy::default());

    if caps.supports_conversation || caps.supports_memory {
        strategy.enable_conversation_memory = true;
        strategy.enable_context_awareness = true;
    }
    if caps.supports_tools || caps.supports_agent {
        strategy.enable_context_awareness = true;
    }
    if caps.supports_agent {
        strategy.enable_response_adaptation = true;
    }
    if caps.supports_memory {
        strategy.enable_conversation_memory = true;
    }
    if provider == "mcp" || profile.framework == "mcp" {
        strategy.enable_context_awareness = true;
        strategy.enable_cross_category_mutation = true;
    }
    if caps.supports_conversation {
        strategy.enable_context_awareness = true;
    }

    strategy
}

pub fn payload_strategy_for_attack_profile(
    profile_id: &str,
    recommended: &PayloadStrategy,
) -> PayloadStrategy {
    match profile_id.trim().to_ascii_lowercase().as_str() {
        "quick" => PayloadStrategy {
            strategy: PayloadGenerationStrategy::Deterministic,
            mutation_level: MutationLevel::Low,
            variants_per_test: 2,
            max_total_payloads: 10,
            enable_context_awareness: false,
            enable_conversation_memory: false,
            enable_response_adaptation: false,
            enable_payload_deduplication: true,
            enable_cross_category_mutation: false,
        },
        "deep" | "red_team" => PayloadStrategy {
            strategy: PayloadGenerationStrategy::Adaptive,
            mutation_level: MutationLevel::Extreme,
            variants_per_test: 10,
            max_total_payloads: recommended.max_total_payloads.max(PAYLOAD_BUDGET_MAX),
            enable_context_awareness: true,
            enable_conversation_memory: true,
            enable_response_adaptation: true,
            enable_payload_deduplication: true,
            enable_cross_category_mutation: true,
        },
        "custom" => recommended.clone(),
        _ => PayloadStrategy {
            strategy: PayloadGenerationStrategy::Mutation,
            mutation_level: MutationLevel::Medium,
            variants_per_test: 5,
            max_total_payloads: recommended.max_total_payloads,
            enable_context_awareness: recommended.enable_context_awareness,
            enable_conversation_memory: recommended.enable_conversation_memory,
            enable_response_adaptation: false,
            enable_payload_deduplication: true,
            enable_cross_category_mutation: false,
        },
    }
}

impl PayloadStrategy {
    pub fn clamp(mut self) -> Self {
        self.variants_per_test = self.variants_per_test.clamp(1, 20);
        self.max_total_payloads = self.max_total_payloads.clamp(PAYLOAD_BUDGET_MIN, PAYLOAD_BUDGET_MAX);
        self
    }

    /// Maps to legacy generator mode for Step 5 execution.
    pub fn generator_mode_str(&self) -> &'static str {
        match self.strategy {
            PayloadGenerationStrategy::Deterministic => "static_pack",
            PayloadGenerationStrategy::Mutation => "template_mutation",
            PayloadGenerationStrategy::Adaptive => "template_mutation",
        }
    }

    pub fn max_variants_per_payload(&self) -> usize {
        let base = self.variants_per_test as usize;
        match self.mutation_level {
            MutationLevel::Low => base.min(2),
            MutationLevel::Medium => base.min(5),
            MutationLevel::High => base.min(10),
            MutationLevel::Extreme => base.min(20),
        }
    }

    pub fn estimate_variants_per_category(&self) -> u32 {
        self.variants_per_test
    }
}

pub fn capability_influences_strategy(caps: &TargetCapabilities, provider: &str, framework: &str) -> Vec<String> {
    let mut notes = Vec::new();
    if caps.supports_tools || caps.supports_agent {
        notes.push("tools/agent → context-aware payloads".into());
    }
    if caps.supports_memory {
        notes.push("memory → conversation memory enabled".into());
    }
    if caps.supports_conversation {
        notes.push("conversation → context-aware payloads".into());
    }
    if provider == "mcp" || framework == "mcp" {
        notes.push("MCP → cross-category mutation recommended".into());
    }
    if caps.supports_agent {
        notes.push("agent → adaptive response evolution".into());
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_profile_uses_deterministic() {
        let s = payload_strategy_for_attack_profile("quick", &PayloadStrategy::default());
        assert_eq!(s.strategy, PayloadGenerationStrategy::Deterministic);
        assert_eq!(s.variants_per_test, 2);
    }

    #[test]
    fn red_team_profile_enables_adaptive() {
        let s = payload_strategy_for_attack_profile("deep", &PayloadStrategy::default());
        assert_eq!(s.strategy, PayloadGenerationStrategy::Adaptive);
        assert!(s.enable_cross_category_mutation);
    }
}
