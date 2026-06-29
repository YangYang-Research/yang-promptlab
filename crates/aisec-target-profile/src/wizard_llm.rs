//! LLM-assisted wizard attack planning from verified API request/response.

use aisec_attack::AttackCategory;
use aisec_planner::types::CategoryRationale;
use aisec_planner::{parse_attack_category, PlannerLlm, PlannerResult};
use serde::Deserialize;
use std::collections::HashMap;

use crate::capabilities::{effective_capabilities, merge_capabilities_into};
use crate::payload_strategy::{
    payload_strategy_for_attack_profile, recommend_payload_strategy, MutationLevel,
    PayloadGenerationStrategy, PayloadStrategy,
};
use crate::planner::plan_from_target_profile;
use crate::prompt::replace_prompt;
use crate::types::{TargetCapabilities, TargetProfile};
use crate::verification::VERIFY_PROBE;
use crate::wizard_plan::{
    build_deterministic_profile_modes, build_wizard_attack_plan, find_profile_mode,
    rebuild_wizard_plan_from_analysis, union_mode_categories, AttackProfileMode,
    ExecutionStrategy, WizardAttackPlan,
};

const MAX_PROMPT_CHARS: usize = 6_000;
const PRESET_PROFILE_IDS: [&str; 3] = ["quick", "standard", "deep"];

#[derive(Debug, Deserialize)]
struct LlmWizardCapabilities {
    #[serde(default, rename = "supportsStreaming")]
    supports_streaming: bool,
    #[serde(default, rename = "supportsTools")]
    supports_tools: bool,
    #[serde(default, rename = "supportsConversation")]
    supports_conversation: bool,
    #[serde(default, rename = "supportsAttachments")]
    supports_attachments: bool,
    #[serde(default, rename = "supportsMemory")]
    supports_memory: bool,
    #[serde(default, rename = "supportsAgent")]
    supports_agent: bool,
}

#[derive(Debug, Deserialize)]
struct LlmWizardRationale {
    category: String,
    reason: String,
    #[serde(default = "default_priority")]
    priority: u8,
}

#[derive(Debug, Deserialize, Default)]
struct LlmWizardPayloadStrategy {
    #[serde(default)]
    strategy: String,
    #[serde(default, rename = "mutationLevel")]
    mutation_level: String,
    #[serde(default, rename = "variantsPerTest")]
    variants_per_test: u32,
    #[serde(default, rename = "maxTotalPayloads")]
    max_total_payloads: u32,
    #[serde(default, rename = "enableContextAwareness")]
    enable_context_awareness: bool,
    #[serde(default, rename = "enableConversationMemory")]
    enable_conversation_memory: bool,
    #[serde(default, rename = "enableResponseAdaptation")]
    enable_response_adaptation: bool,
    #[serde(default, rename = "enablePayloadDeduplication")]
    enable_payload_deduplication: bool,
    #[serde(default, rename = "enableCrossCategoryMutation")]
    enable_cross_category_mutation: bool,
}

#[derive(Debug, Deserialize)]
struct LlmWizardMode {
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default, rename = "executionStrategy")]
    execution_strategy: String,
    #[serde(default, rename = "maxAttempts")]
    max_attempts: Option<u8>,
    #[serde(default, rename = "reflectionEnabled")]
    reflection_enabled: Option<bool>,
    #[serde(default, rename = "adaptivePlanning")]
    adaptive_planning: Option<bool>,
    #[serde(default, rename = "payloadStrategy")]
    payload_strategy: Option<LlmWizardPayloadStrategy>,
    #[serde(default)]
    rationales: Vec<LlmWizardRationale>,
}

#[derive(Debug, Deserialize)]
struct LlmWizardPlanResponse {
    #[serde(default, rename = "recommendedProfileId")]
    recommended_profile_id: Option<String>,
    profile_id: Option<String>,
    categories: Option<Vec<String>>,
    disabled_tests: Option<Vec<String>>,
    capabilities: Option<LlmWizardCapabilities>,
    rationales: Option<Vec<LlmWizardRationale>>,
    rationale: Option<String>,
    #[serde(default)]
    modes: HashMap<String, LlmWizardMode>,
}

struct WizardLlmRefinement {
    recommended_profile_id: String,
    profile_modes: Vec<AttackProfileMode>,
    suggested_categories: Vec<AttackCategory>,
    disabled_tests: Vec<String>,
    rationales: Vec<CategoryRationale>,
    capabilities: Option<TargetCapabilities>,
}

fn default_priority() -> u8 {
    2
}

/// Builds a wizard attack plan, optionally refined by AI Runtime analysis of verify traffic.
pub async fn build_wizard_attack_plan_with_llm(
    profile: &TargetProfile,
    llm: Option<&dyn PlannerLlm>,
) -> WizardAttackPlan {
    let baseline = build_wizard_attack_plan(profile);
    let Some(llm) = llm else {
        return baseline;
    };
    if !profile.is_verified() {
        return baseline;
    }

    let response_preview = profile
        .verification
        .response_preview
        .as_deref()
        .unwrap_or("")
        .trim();
    if response_preview.is_empty() {
        return baseline;
    }

    let request_body = replace_prompt(
        &profile.request_template,
        &profile.prompt_placeholder,
        VERIFY_PROBE,
    );

    let allowed: Vec<_> = AttackCategory::all().iter().map(|c| c.as_str()).collect();
    let baseline_cats: Vec<_> = baseline
        .suggested_categories
        .iter()
        .map(|c| c.as_str())
        .collect();
    let detected_model = profile
        .verification
        .model
        .as_deref()
        .unwrap_or("unknown");

    let prompt = aisec_inference::PromptRegistry::wizard_profile_user(
        profile.provider.as_str(),
        &profile.framework,
        &profile.full_url(),
        &format!("{allowed:?}"),
        &format!("{baseline_cats:?}"),
        &truncate_for_prompt(&request_body),
        &truncate_for_prompt(response_preview),
        detected_model,
    );

    match llm.complete(&prompt).await {
        Ok(raw) => match parse_wizard_llm_plan(&raw, profile, &baseline) {
            Ok(refinement) => apply_wizard_llm_refinement(profile, baseline, refinement),
            Err(_) => baseline,
        },
        Err(_) => baseline,
    }
}

fn truncate_for_prompt(text: &str) -> String {
    if text.len() <= MAX_PROMPT_CHARS {
        return text.to_string();
    }
    format!("{}… [truncated]", &text[..MAX_PROMPT_CHARS])
}

fn parse_wizard_llm_plan(
    raw: &str,
    profile: &TargetProfile,
    baseline: &WizardAttackPlan,
) -> PlannerResult<WizardLlmRefinement> {
    let json_str = extract_json_object(raw)?;
    let parsed: LlmWizardPlanResponse = serde_json::from_str(&json_str)
        .map_err(|e| aisec_planner::PlannerError::Llm(format!("invalid JSON: {e}")))?;

    let mut enhanced = profile.clone();
    if let Some(ref llm_caps) = parsed.capabilities {
        let caps = TargetCapabilities {
            supports_streaming: llm_caps.supports_streaming,
            supports_tools: llm_caps.supports_tools,
            supports_conversation: llm_caps.supports_conversation,
            supports_attachments: llm_caps.supports_attachments,
            supports_memory: llm_caps.supports_memory,
            supports_agent: llm_caps.supports_agent,
        };
        merge_capabilities_into(&mut enhanced.default_capabilities, &caps);
    }

    let deterministic = plan_from_target_profile(&enhanced);
    let applicable = deterministic.categories.clone();
    let applicable_set: std::collections::HashSet<_> = applicable.iter().copied().collect();
    let fallback_modes = build_deterministic_profile_modes(&applicable, profile);

    let mut profile_modes = if parsed.modes.is_empty() {
        build_legacy_profile_modes(&parsed, &fallback_modes, &applicable_set)?
    } else {
        build_modes_from_llm(&parsed.modes, &fallback_modes, &applicable_set)
    };

    if profile_modes.is_empty() {
        profile_modes = fallback_modes;
    }

    let recommended_profile_id = parsed
        .recommended_profile_id
        .or(parsed.profile_id)
        .map(|value| normalize_profile_id(&value))
        .unwrap_or_else(|| baseline.recommended_profile_id.clone());

    let suggested_categories = union_mode_categories(&profile_modes);
    let suggested_categories = if suggested_categories.is_empty() {
        applicable
    } else {
        suggested_categories
    };

    let mut rationales = parsed
        .rationales
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| {
            let category = parse_attack_category(&item.category)?;
            Some(CategoryRationale {
                category,
                reason: item.reason,
                priority: item.priority,
                source: "ai_runtime".into(),
            })
        })
        .collect::<Vec<_>>();

    if rationales.is_empty() {
        if let Some(llm_mode) = parsed.modes.get(&recommended_profile_id) {
            for rationale in &llm_mode.rationales {
                if let Some(category) = parse_attack_category(&rationale.category) {
                    rationales.push(CategoryRationale {
                        category,
                        reason: rationale.reason.clone(),
                        priority: rationale.priority,
                        source: "ai_runtime".into(),
                    });
                }
            }
        }
    }

    if rationales.is_empty() {
        if let Some(summary) = parsed.rationale.filter(|s| !s.is_empty()) {
            for category in &suggested_categories {
                rationales.push(CategoryRationale {
                    category: *category,
                    reason: summary.clone(),
                    priority: 2,
                    source: "ai_runtime".into(),
                });
            }
        }
    }

    Ok(WizardLlmRefinement {
        recommended_profile_id,
        profile_modes,
        suggested_categories,
        disabled_tests: parsed.disabled_tests.unwrap_or_default(),
        rationales,
        capabilities: Some(effective_capabilities(&enhanced)),
    })
}

fn build_legacy_profile_modes(
    parsed: &LlmWizardPlanResponse,
    fallback_modes: &[AttackProfileMode],
    applicable_set: &std::collections::HashSet<AttackCategory>,
) -> PlannerResult<Vec<AttackProfileMode>> {
    let profile_id = normalize_profile_id(
        parsed
            .recommended_profile_id
            .as_deref()
            .or(parsed.profile_id.as_deref())
            .unwrap_or("standard"),
    );
    let categories = parsed
        .categories
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|raw| parse_attack_category(&raw))
        .filter(|category| applicable_set.contains(category))
        .collect::<Vec<_>>();

    let mut modes = fallback_modes.to_vec();
    if let Some(mode) = find_profile_mode_mut(&mut modes, &profile_id) {
        if !categories.is_empty() {
            mode.categories = categories;
        }
    }
    Ok(modes)
}

fn build_modes_from_llm(
    modes: &HashMap<String, LlmWizardMode>,
    fallback_modes: &[AttackProfileMode],
    applicable_set: &std::collections::HashSet<AttackCategory>,
) -> Vec<AttackProfileMode> {
    PRESET_PROFILE_IDS
        .iter()
        .filter_map(|profile_id| {
            let fallback = find_profile_mode(fallback_modes, profile_id)?;
            let llm_mode = modes.get(*profile_id);
            Some(merge_profile_mode(profile_id, fallback, llm_mode, applicable_set))
        })
        .collect()
}

fn merge_profile_mode(
    profile_id: &str,
    fallback: &AttackProfileMode,
    llm_mode: Option<&LlmWizardMode>,
    applicable_set: &std::collections::HashSet<AttackCategory>,
) -> AttackProfileMode {
    let Some(llm_mode) = llm_mode else {
        return fallback.clone();
    };

    let categories = llm_mode
        .categories
        .iter()
        .filter_map(|raw| parse_attack_category(raw))
        .filter(|category| applicable_set.contains(category))
        .collect::<Vec<_>>();

    AttackProfileMode {
        profile_id: profile_id.into(),
        categories: if categories.is_empty() {
            fallback.categories.clone()
        } else {
            categories
        },
        execution_strategy: parse_execution_strategy(&llm_mode.execution_strategy)
            .unwrap_or(fallback.execution_strategy),
        max_attempts: llm_mode
            .max_attempts
            .unwrap_or(fallback.max_attempts)
            .clamp(1, 20),
        reflection_enabled: llm_mode
            .reflection_enabled
            .unwrap_or(fallback.reflection_enabled),
        adaptive_planning: llm_mode
            .adaptive_planning
            .unwrap_or(fallback.adaptive_planning),
        payload_strategy: llm_mode
            .payload_strategy
            .as_ref()
            .map(|value| parse_payload_strategy(value, &fallback.payload_strategy))
            .unwrap_or_else(|| fallback.payload_strategy.clone())
            .clamp(),
    }
}

fn parse_execution_strategy(raw: &str) -> Option<ExecutionStrategy> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "agentic" => Some(ExecutionStrategy::Agentic),
        "sequential" => Some(ExecutionStrategy::Sequential),
        _ => None,
    }
}

fn parse_payload_strategy(
    raw: &LlmWizardPayloadStrategy,
    fallback: &PayloadStrategy,
) -> PayloadStrategy {
    let strategy = match raw.strategy.trim().to_ascii_lowercase().as_str() {
        "deterministic" => PayloadGenerationStrategy::Deterministic,
        "adaptive" => PayloadGenerationStrategy::Adaptive,
        "mutation" => PayloadGenerationStrategy::Mutation,
        _ => fallback.strategy,
    };
    let mutation_level = match raw.mutation_level.trim().to_ascii_lowercase().as_str() {
        "low" => MutationLevel::Low,
        "high" => MutationLevel::High,
        "extreme" => MutationLevel::Extreme,
        "medium" => MutationLevel::Medium,
        _ => fallback.mutation_level,
    };
    let variants_per_test = if raw.variants_per_test == 0 {
        fallback.variants_per_test
    } else {
        raw.variants_per_test
    };
    let max_total_payloads = if raw.max_total_payloads == 0 {
        fallback.max_total_payloads
    } else {
        raw.max_total_payloads
    };

    PayloadStrategy {
        strategy,
        mutation_level,
        variants_per_test,
        max_total_payloads,
        enable_context_awareness: raw.enable_context_awareness || fallback.enable_context_awareness,
        enable_conversation_memory: raw.enable_conversation_memory
            || fallback.enable_conversation_memory,
        enable_response_adaptation: raw.enable_response_adaptation
            || fallback.enable_response_adaptation,
        enable_payload_deduplication: if raw.enable_payload_deduplication {
            true
        } else {
            fallback.enable_payload_deduplication
        },
        enable_cross_category_mutation: raw.enable_cross_category_mutation
            || fallback.enable_cross_category_mutation,
    }
}

fn find_profile_mode_mut<'a>(
    modes: &'a mut [AttackProfileMode],
    profile_id: &str,
) -> Option<&'a mut AttackProfileMode> {
    let key = profile_id.trim().to_ascii_lowercase();
    modes
        .iter_mut()
        .find(|mode| mode.profile_id.eq_ignore_ascii_case(&key))
}

fn extract_json_object(raw: &str) -> PlannerResult<String> {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') {
        return Ok(trimmed.to_string());
    }
    let start = trimmed
        .find('{')
        .ok_or_else(|| aisec_planner::PlannerError::Llm("no JSON object in LLM response".into()))?;
    let end = trimmed
        .rfind('}')
        .ok_or_else(|| aisec_planner::PlannerError::Llm("unterminated JSON in LLM response".into()))?;
    Ok(trimmed[start..=end].to_string())
}

fn apply_wizard_llm_refinement(
    profile: &TargetProfile,
    mut plan: WizardAttackPlan,
    mut refinement: WizardLlmRefinement,
) -> WizardAttackPlan {
    let mut enhanced = profile.clone();
    if let Some(llm_caps) = refinement.capabilities {
        merge_capabilities_into(&mut enhanced.default_capabilities, &llm_caps);
    }

    let deterministic = plan_from_target_profile(&enhanced);
    let caps = effective_capabilities(&enhanced);
    plan.disabled_tests = refinement.disabled_tests;
    plan.recommended_payload_strategy = recommend_payload_strategy(&enhanced);

    for mode in &mut refinement.profile_modes {
        if mode.payload_strategy.strategy == PayloadGenerationStrategy::Mutation
            && mode.payload_strategy.max_total_payloads == 0
        {
            mode.payload_strategy = payload_strategy_for_attack_profile(
                &mode.profile_id,
                &plan.recommended_payload_strategy,
            )
            .clamp();
        }
    }

    rebuild_wizard_plan_from_analysis(
        profile,
        plan,
        refinement.profile_modes,
        refinement.recommended_profile_id,
        refinement.suggested_categories,
        refinement.rationales,
        &caps,
        deterministic.confidence,
    )
}

fn normalize_profile_id(profile_id: &str) -> String {
    match profile_id.trim().to_ascii_lowercase().as_str() {
        "quick" | "standard" | "deep" | "custom" => profile_id.trim().to_ascii_lowercase(),
        "red_team" => "deep".into(),
        _ => "standard".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::collections::HashMap;

    struct MockLlm(&'static str);

    #[async_trait]
    impl PlannerLlm for MockLlm {
        async fn complete(&self, _prompt: &str) -> PlannerResult<String> {
            Ok(self.0.to_string())
        }
    }

    fn sample_profile() -> TargetProfile {
        TargetProfile {
            provider: crate::types::TargetProvider::GenericHttp,
            framework: "openai".into(),
            method: crate::types::HttpMethod::Post,
            base_url: "https://api.example.com".into(),
            path: "/v1/chat".into(),
            headers: HashMap::new(),
            request_template: r#"{ "messages": [{ "role": "user", "content": "{{PROMPT}}" }] }"#
                .into(),
            prompt_placeholder: "{{PROMPT}}".into(),
            default_capabilities: TargetCapabilities::default(),
            verification_strategy: "generic_http".into(),
            verification: crate::types::VerificationResult {
                verified: true,
                provider: "generic_http".into(),
                response_preview: Some(r#"{"choices":[{"message":{"content":"Hello!"}}]}"#.into()),
                status: "verified".into(),
                ..crate::types::VerificationResult {
                    verified: false,
                    verified_at: None,
                    provider: String::new(),
                    model: None,
                    capabilities: TargetCapabilities::default(),
                    response_time_ms: 0,
                    status_code: 200,
                    status: String::new(),
                    response_preview: None,
                    error_message: None,
                }
            },
            ..TargetProfile::default()
        }
    }

    #[tokio::test]
    async fn llm_refines_modes_when_runtime_available() {
        let llm_json = r#"{
          "recommendedProfileId": "standard",
          "capabilities": {
            "supportsTools": true,
            "supportsConversation": true,
            "supportsMemory": true,
            "supportsAgent": true
          },
          "modes": {
            "quick": {
              "categories": ["prompt_injection", "jailbreak"],
              "executionStrategy": "sequential",
              "maxAttempts": 3,
              "payloadStrategy": {
                "strategy": "deterministic",
                "mutationLevel": "low",
                "variantsPerTest": 2,
                "maxTotalPayloads": 10
              }
            },
            "standard": {
              "categories": ["prompt_injection", "jailbreak", "tool_abuse"],
              "executionStrategy": "sequential",
              "payloadStrategy": {
                "strategy": "mutation",
                "mutationLevel": "medium",
                "variantsPerTest": 5,
                "maxTotalPayloads": 20
              },
              "rationales": [
                { "category": "tool_abuse", "reason": "tools in request schema", "priority": 1 }
              ]
            },
            "deep": {
              "categories": ["prompt_injection", "jailbreak", "tool_abuse", "agent_goal_hijacking"],
              "executionStrategy": "agentic",
              "maxAttempts": 5,
              "reflectionEnabled": true,
              "adaptivePlanning": true,
              "payloadStrategy": {
                "strategy": "adaptive",
                "mutationLevel": "extreme",
                "variantsPerTest": 10,
                "maxTotalPayloads": 100,
                "enableResponseAdaptation": true,
                "enableCrossCategoryMutation": true
              }
            }
          }
        }"#;

        let plan =
            build_wizard_attack_plan_with_llm(&sample_profile(), Some(&MockLlm(llm_json))).await;
        assert_eq!(plan.profile_modes.len(), 3);
        assert!(plan.suggested_categories.contains(&AttackCategory::ToolAbuse));
        assert!(plan.rationales.iter().any(|r| r.source == "ai_runtime"));
        let standard = find_profile_mode(&plan.profile_modes, "standard").expect("standard mode");
        assert!(standard.categories.contains(&AttackCategory::ToolAbuse));
        let deep = find_profile_mode(&plan.profile_modes, "deep").expect("deep mode");
        assert_eq!(deep.execution_strategy, ExecutionStrategy::Agentic);
        assert_eq!(
            deep.payload_strategy.strategy,
            PayloadGenerationStrategy::Adaptive
        );
    }

    #[test]
    fn normalize_profile_id_maps_red_team_to_deep() {
        assert_eq!(normalize_profile_id("red_team"), "deep");
        assert_eq!(normalize_profile_id("quick"), "quick");
    }
}
