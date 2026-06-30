//! Wizard attack plan — capability-driven planning output for Scan Wizard Step 4.

use std::collections::HashSet;

use aisec_attack::AttackCategory;
use aisec_planner::types::{AttackPlan, CategoryRationale, PlannerMode};
use serde::{Deserialize, Serialize};

use crate::payload_strategy::{
    payload_strategy_for_attack_profile, recommend_payload_strategy, PayloadStrategy,
};
use crate::types::{TargetCapabilities, TargetProfile};

const PAYLOADS_PER_CATEGORY: u32 = 3;
const TESTS_PER_CATEGORY: u32 = 3;
const SECONDS_PER_REQUEST: f32 = 2.5;
const TOKENS_PER_REQUEST: u32 = 480;
const CATALOG_SIZE: u32 = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStrategy {
    Sequential,
    Agentic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttackGraphNode {
    pub category: AttackCategory,
    pub priority: u8,
    pub risk: u8,
    pub confidence: f32,
    pub dependencies: Vec<AttackCategory>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttackProfileMode {
    pub profile_id: String,
    pub categories: Vec<AttackCategory>,
    pub execution_strategy: ExecutionStrategy,
    pub max_attempts: u8,
    pub reflection_enabled: bool,
    pub adaptive_planning: bool,
    pub payload_strategy: PayloadStrategy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WizardAttackPlan {
    pub profile_id: String,
    pub recommended_profile_id: String,
    pub suggested_categories: Vec<AttackCategory>,
    pub profile_modes: Vec<AttackProfileMode>,
    pub categories: Vec<AttackCategory>,
    pub disabled_tests: Vec<String>,
    pub capability_graph: Vec<String>,
    pub attack_graph: Vec<AttackGraphNode>,
    pub execution_strategy: ExecutionStrategy,
    pub max_attempts: u8,
    pub reflection_enabled: bool,
    pub adaptive_planning: bool,
    pub rationales: Vec<CategoryRationale>,
    pub confidence: f32,
    pub summary: String,
    pub risk_score: u8,
    pub risk_level: String,
    pub estimated_requests: u32,
    pub estimated_runtime_seconds: u32,
    pub estimated_tokens: u32,
    pub coverage_score: f32,
    pub risk_coverage: f32,
    pub total_testcases: u32,
    pub payload_strategy: PayloadStrategy,
    pub recommended_payload_strategy: PayloadStrategy,
    /// `ai_runtime` when all preset modes were produced by AI Runtime; otherwise `target_profile`.
    pub planner_source: String,
}

pub fn profile_categories_for_id(profile_id: &str) -> Vec<AttackCategory> {
    match profile_id.trim().to_ascii_lowercase().as_str() {
        "quick" => vec![
            AttackCategory::PromptInjection,
            AttackCategory::Jailbreak,
            AttackCategory::SystemPromptExtraction,
        ],
        "deep" | "red_team" => AttackCategory::all().to_vec(),
        "custom" => vec![],
        _ => vec![
            AttackCategory::PromptInjection,
            AttackCategory::Jailbreak,
            AttackCategory::SystemPromptExtraction,
            AttackCategory::RagLeakage,
            AttackCategory::ToolAbuse,
            AttackCategory::CrossUserLeakage,
        ],
    }
}

pub fn build_deterministic_profile_modes(
    suggested: &[AttackCategory],
    profile: &TargetProfile,
) -> Vec<AttackProfileMode> {
    let recommended_base = recommend_payload_strategy(profile);
    let suggested_set: HashSet<_> = suggested.iter().copied().collect();

    ["quick", "standard", "deep"]
        .into_iter()
        .map(|profile_id| {
            let preset = profile_categories_for_id(profile_id);
            let categories: Vec<_> = preset
                .into_iter()
                .filter(|category| suggested_set.contains(category))
                .collect();
            let payload =
                payload_strategy_for_attack_profile(profile_id, &recommended_base).clamp();
            let (execution_strategy, max_attempts, reflection_enabled, adaptive_planning) =
                match profile_id {
                    "deep" => (ExecutionStrategy::Agentic, 5_u8, true, true),
                    "standard" => (ExecutionStrategy::Sequential, 5, false, false),
                    _ => (ExecutionStrategy::Sequential, 3, false, false),
                };
            AttackProfileMode {
                profile_id: profile_id.into(),
                categories,
                execution_strategy,
                max_attempts,
                reflection_enabled,
                adaptive_planning,
                payload_strategy: payload,
            }
        })
        .collect()
}

pub fn union_mode_categories(modes: &[AttackProfileMode]) -> Vec<AttackCategory> {
    let mut seen = HashSet::new();
    let mut ordered = Vec::new();
    for mode in modes {
        for category in &mode.categories {
            if seen.insert(*category) {
                ordered.push(*category);
            }
        }
    }
    ordered
}

pub fn find_profile_mode<'a>(
    modes: &'a [AttackProfileMode],
    profile_id: &str,
) -> Option<&'a AttackProfileMode> {
    let key = profile_id.trim().to_ascii_lowercase();
    modes
        .iter()
        .find(|mode| mode.profile_id.eq_ignore_ascii_case(&key))
}

pub fn apply_profile_mode_settings(plan: &mut WizardAttackPlan, mode: &AttackProfileMode) {
    plan.profile_id = mode.profile_id.clone();
    plan.categories = mode.categories.clone();
    plan.execution_strategy = mode.execution_strategy;
    plan.max_attempts = mode.max_attempts;
    plan.reflection_enabled = mode.reflection_enabled;
    plan.adaptive_planning = mode.adaptive_planning;
    plan.payload_strategy = mode.payload_strategy.clone();
}

pub fn build_wizard_attack_plan(profile: &TargetProfile) -> WizardAttackPlan {
    let caps = crate::capabilities::effective_capabilities(profile);
    let base = super::planner::plan_from_target_profile(profile);
    let capability_graph = capability_labels(&caps, &profile.framework, profile.provider.as_str());
    let suggested = base.categories.clone();
    let profile_modes = build_deterministic_profile_modes(&suggested, profile);
    let attack_graph = build_attack_graph(&base.rationales, &suggested);
    let risk_score = compute_risk_score(&caps, suggested.len());
    let risk_level = risk_level_label(risk_score).to_string();

    let recommended = recommend_payload_strategy(profile);

    let mut plan = WizardAttackPlan {
        profile_id: "standard".into(),
        recommended_profile_id: "standard".into(),
        suggested_categories: suggested.clone(),
        profile_modes,
        categories: Vec::new(),
        disabled_tests: vec![],
        capability_graph,
        attack_graph,
        execution_strategy: ExecutionStrategy::Sequential,
        max_attempts: 5,
        reflection_enabled: false,
        adaptive_planning: false,
        rationales: base.rationales,
        confidence: base.confidence,
        summary: base.summary,
        risk_score,
        risk_level,
        estimated_requests: 0,
        estimated_runtime_seconds: 0,
        estimated_tokens: 0,
        coverage_score: 0.0,
        risk_coverage: 0.0,
        total_testcases: 0,
        payload_strategy: recommended.clone(),
        recommended_payload_strategy: recommended,
        planner_source: "target_profile".into(),
    };
    if let Some(mode) = find_profile_mode(&plan.profile_modes, "standard").cloned() {
        apply_profile_mode_settings(&mut plan, &mode);
    }
    recompute_estimates(&mut plan);
    plan.summary = build_wizard_plan_summary(&plan, &profile.full_url());
    plan
}

pub(crate) fn rebuild_wizard_plan_from_analysis(
    profile: &TargetProfile,
    mut plan: WizardAttackPlan,
    profile_modes: Vec<AttackProfileMode>,
    recommended_profile_id: String,
    suggested: Vec<AttackCategory>,
    rationales: Vec<CategoryRationale>,
    caps: &TargetCapabilities,
    confidence: f32,
) -> WizardAttackPlan {
    plan.profile_modes = profile_modes;
    plan.recommended_profile_id = recommended_profile_id.clone();
    plan.suggested_categories = suggested.clone();
    plan.capability_graph = capability_labels(caps, &profile.framework, profile.provider.as_str());
    plan.attack_graph = build_attack_graph(&rationales, &suggested);
    plan.rationales = rationales;
    plan.confidence = plan.confidence.max(confidence).min(1.0);
    plan.risk_score = compute_risk_score(caps, suggested.len());
    plan.risk_level = risk_level_label(plan.risk_score).to_string();

    if let Some(mode) = find_profile_mode(&plan.profile_modes, &recommended_profile_id).cloned() {
        apply_profile_mode_settings(&mut plan, &mode);
    } else {
        plan.profile_id = recommended_profile_id;
        plan.categories =
            active_categories_for_profile(&plan.profile_id, &suggested, &plan.profile_modes);
    }

    recompute_estimates(&mut plan);
    plan.summary = build_wizard_plan_summary(&plan, &profile.full_url());
    plan
}

pub fn adjust_wizard_attack_plan(
    mut plan: WizardAttackPlan,
    profile_id: &str,
    categories: Option<Vec<AttackCategory>>,
    disabled_tests: &[String],
    disabled_graph_nodes: &[String],
    payload_strategy: Option<PayloadStrategy>,
) -> WizardAttackPlan {
    plan.profile_id = profile_id.to_string();
    plan.disabled_tests = disabled_tests.to_vec();

    let disabled_cats: HashSet<String> = disabled_graph_nodes
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();

    if profile_id.eq_ignore_ascii_case("custom") {
        plan.categories = categories.unwrap_or_else(|| plan.suggested_categories.clone());
        if let Some(strategy) = payload_strategy {
            plan.payload_strategy = strategy.clamp();
        }
    } else if let Some(mode) = find_profile_mode(&plan.profile_modes, profile_id).cloned() {
        apply_profile_mode_settings(&mut plan, &mode);
        if let Some(strategy) = payload_strategy {
            plan.payload_strategy = strategy.clamp();
        }
    } else {
        plan.categories =
            active_categories_for_profile(profile_id, &plan.suggested_categories, &plan.profile_modes);
        if let Some(strategy) = payload_strategy {
            plan.payload_strategy = strategy.clamp();
        } else {
            plan.payload_strategy =
                payload_strategy_for_attack_profile(profile_id, &plan.recommended_payload_strategy)
                    .clamp();
        }
    }

    plan.categories.retain(|c| !disabled_cats.contains(c.as_str()));

    for node in &mut plan.attack_graph {
        node.enabled = plan.categories.contains(&node.category);
    }

    recompute_estimates(&mut plan);

    let active: HashSet<_> = plan.categories.iter().copied().collect();
    plan.rationales.retain(|r| active.contains(&r.category));
    plan.rationales.sort_by_key(|r| r.priority);

    plan
}

/// Human-readable planner summary for wizard Step 4 — endpoint label only; metrics live in the UI grid.
pub fn build_wizard_plan_summary(plan: &WizardAttackPlan, api_endpoint: &str) -> String {
    let _ = plan;
    format!("Plan for {api_endpoint}")
}

pub fn active_categories_for_profile(
    profile_id: &str,
    suggested: &[AttackCategory],
    profile_modes: &[AttackProfileMode],
) -> Vec<AttackCategory> {
    if profile_id.eq_ignore_ascii_case("custom") {
        return suggested.to_vec();
    }
    if let Some(mode) = find_profile_mode(profile_modes, profile_id) {
        let suggested_set: HashSet<_> = suggested.iter().copied().collect();
        return mode
            .categories
            .iter()
            .copied()
            .filter(|category| suggested_set.contains(category))
            .collect();
    }
    let preset = profile_categories_for_id(profile_id);
    let suggested_set: HashSet<_> = suggested.iter().copied().collect();
    preset
        .into_iter()
        .filter(|c| suggested_set.contains(c))
        .collect()
}

fn capability_labels(caps: &TargetCapabilities, framework: &str, provider: &str) -> Vec<String> {
    let mut labels = vec![format!("provider:{provider}"), format!("framework:{framework}")];
    if caps.supports_streaming {
        labels.push("streaming".into());
    }
    if caps.supports_conversation {
        labels.push("conversation".into());
    }
    if caps.supports_memory {
        labels.push("memory".into());
    }
    if caps.supports_tools {
        labels.push("tools".into());
    }
    if caps.supports_agent {
        labels.push("agent".into());
    }
    if provider == "mcp" || framework == "mcp" {
        labels.push("mcp".into());
    }
    labels
}

fn build_attack_graph(
    rationales: &[CategoryRationale],
    categories: &[AttackCategory],
) -> Vec<AttackGraphNode> {
    let mut ordered: Vec<_> = categories
        .iter()
        .map(|cat| {
            let rationale = rationales
                .iter()
                .find(|r| r.category == *cat)
                .map(|r| (r.priority, r.reason.clone()))
                .unwrap_or((5, String::new()));
            (*cat, rationale.0, rationale.1)
        })
        .collect();
    ordered.sort_by_key(|(_, priority, _)| *priority);

    let mut nodes = Vec::new();
    let mut prev: Option<AttackCategory> = None;
    for (category, priority, _) in ordered {
        let risk = category_risk(category);
        let confidence = category_confidence(category);
        let dependencies = prev.map(|p| vec![p]).unwrap_or_default();
        nodes.push(AttackGraphNode {
            category,
            priority,
            risk,
            confidence,
            dependencies,
            enabled: true,
        });
        prev = Some(category);
    }
    nodes
}

fn category_risk(category: AttackCategory) -> u8 {
    match category {
        AttackCategory::PromptInjection | AttackCategory::ToolAbuse | AttackCategory::McpAbuse => 90,
        AttackCategory::Jailbreak | AttackCategory::AgentGoalHijacking => 80,
        AttackCategory::SystemPromptExtraction | AttackCategory::MemoryPoisoning => 70,
        AttackCategory::RagLeakage | AttackCategory::CrossUserLeakage => 60,
    }
}

fn category_confidence(category: AttackCategory) -> f32 {
    match category {
        AttackCategory::PromptInjection | AttackCategory::Jailbreak => 0.92,
        AttackCategory::SystemPromptExtraction => 0.88,
        _ => 0.85,
    }
}

fn compute_risk_score(caps: &TargetCapabilities, category_count: usize) -> u8 {
    let mut score: u32 = 40 + (category_count as u32 * 4).min(24);
    if caps.supports_tools || caps.supports_agent {
        score += 12;
    }
    if caps.supports_memory {
        score += 8;
    }
    score.min(95) as u8
}

fn risk_level_label(score: u8) -> &'static str {
    if score >= 75 {
        "high"
    } else if score >= 50 {
        "medium"
    } else {
        "low"
    }
}

pub fn recompute_estimates(plan: &mut WizardAttackPlan) {
    let disabled: HashSet<&str> = plan.disabled_tests.iter().map(String::as_str).collect();
    let mut requests = 0u32;
    let mut total_testcases = 0u32;
    for category in &plan.categories {
        let disabled_in_category = plan
            .disabled_tests
            .iter()
            .filter(|id| id.starts_with(test_prefix_for_category(*category)))
            .count() as u32;
        let enabled_tests = TESTS_PER_CATEGORY.saturating_sub(disabled_in_category);
        if enabled_tests == 0 {
            continue;
        }
        total_testcases += enabled_tests;
        let ratio = enabled_tests as f32 / TESTS_PER_CATEGORY as f32;
        let variants = plan.payload_strategy.variants_per_test;
        requests += ((PAYLOADS_PER_CATEGORY * variants) as f32 * ratio).round() as u32;
    }

    plan.total_testcases = total_testcases;
    let execution_multiplier = match plan.execution_strategy {
        ExecutionStrategy::Sequential => 1_u32,
        ExecutionStrategy::Agentic => plan.max_attempts.max(1) as u32,
    };
    let adjusted_requests = requests.saturating_mul(execution_multiplier);

    plan.estimated_requests = adjusted_requests;
    plan.estimated_runtime_seconds =
        (adjusted_requests as f32 * SECONDS_PER_REQUEST).ceil() as u32;
    plan.estimated_tokens = adjusted_requests * TOKENS_PER_REQUEST;
    plan.coverage_score = if CATALOG_SIZE == 0 {
        0.0
    } else {
        plan.categories.len() as f32 / CATALOG_SIZE as f32
    };
    let enabled_risk: u32 = plan
        .attack_graph
        .iter()
        .filter(|n| plan.categories.contains(&n.category))
        .map(|n| n.risk as u32)
        .sum();
    let total_risk: u32 = plan
        .attack_graph
        .iter()
        .map(|n| n.risk as u32)
        .sum::<u32>()
        .max(1);
    plan.risk_coverage = enabled_risk as f32 / total_risk as f32;
    let _ = disabled; // reserved for per-test disable accounting
}

fn test_prefix_for_category(category: AttackCategory) -> &'static str {
    match category {
        AttackCategory::PromptInjection => "pi-",
        AttackCategory::SystemPromptExtraction => "spe-",
        AttackCategory::Jailbreak => "jb-",
        AttackCategory::RagLeakage => "rag-",
        AttackCategory::MemoryPoisoning => "mp-",
        AttackCategory::CrossUserLeakage => "cul-",
        AttackCategory::AgentGoalHijacking => "agh-",
        AttackCategory::ToolAbuse => "ta-",
        AttackCategory::McpAbuse => "mcp-",
    }
}

impl From<WizardAttackPlan> for AttackPlan {
    fn from(plan: WizardAttackPlan) -> Self {
        AttackPlan {
            mode: PlannerMode::Deterministic,
            profile_id: plan.profile_id,
            categories: plan.categories,
            disabled_tests: plan.disabled_tests,
            rationales: plan.rationales,
            confidence: plan.confidence,
            summary: plan.summary,
            llm_rationale: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TargetCapabilities, TargetProvider};

    fn sample_profile() -> TargetProfile {
        TargetProfile {
            provider: TargetProvider::OpenAiCompatible,
            framework: "openai".into(),
            default_capabilities: TargetCapabilities {
                supports_streaming: true,
                supports_tools: true,
                supports_agent: false,
                supports_memory: false,
                supports_conversation: true,
                supports_attachments: false,
            },
            ..Default::default()
        }
    }

    #[test]
    fn builds_graph_and_estimates() {
        let plan = build_wizard_attack_plan(&sample_profile());
        assert!(!plan.attack_graph.is_empty());
        assert!(plan.estimated_requests > 0);
        assert!(plan.coverage_score > 0.0);
    }

    #[test]
    fn profile_quick_reduces_categories_via_modes() {
        let plan = build_wizard_attack_plan(&sample_profile());
        let full_requests = plan.estimated_requests;
        let quick_count = find_profile_mode(&plan.profile_modes, "quick")
            .map(|mode| mode.categories.len())
            .unwrap_or(0);
        let adjusted = adjust_wizard_attack_plan(plan, "quick", None, &[], &[], None);
        assert_eq!(adjusted.categories.len(), quick_count);
        assert!(adjusted.estimated_requests <= full_requests);
    }

    #[test]
    fn profile_modes_include_all_presets() {
        let plan = build_wizard_attack_plan(&sample_profile());
        assert_eq!(plan.profile_modes.len(), 3);
        assert!(find_profile_mode(&plan.profile_modes, "quick").is_some());
        assert!(find_profile_mode(&plan.profile_modes, "standard").is_some());
        assert!(find_profile_mode(&plan.profile_modes, "deep").is_some());
    }

    #[test]
    fn agentic_execution_increases_request_estimate() {
        let mut plan = build_wizard_attack_plan(&sample_profile());
        let sequential = plan.estimated_requests;
        plan.execution_strategy = ExecutionStrategy::Agentic;
        plan.max_attempts = 5;
        recompute_estimates(&mut plan);
        assert_eq!(plan.estimated_requests, sequential * 5);
    }

    #[test]
    fn summary_is_endpoint_label_only() {
        let plan = build_wizard_attack_plan(&sample_profile());
        let summary = build_wizard_plan_summary(&plan, "https://api.example.com/v1/chat");
        assert_eq!(summary, "Plan for https://api.example.com/v1/chat");
    }
}
