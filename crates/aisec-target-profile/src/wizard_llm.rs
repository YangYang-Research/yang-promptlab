//! LLM-assisted wizard attack planning from verified API request/response.

use aisec_attack::AttackCategory;
use aisec_planner::types::CategoryRationale;
use aisec_planner::{parse_attack_category, PlannerError, PlannerLlm, PlannerResult};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use tracing::warn;

use crate::capabilities::{effective_capabilities, merge_capabilities_into};
use crate::payload_strategy::{
    recommend_payload_strategy, MutationLevel, PayloadGenerationStrategy, PayloadStrategy,
};
use crate::planner::plan_from_target_profile;
use crate::prompt::replace_prompt;
use crate::types::{TargetCapabilities, TargetProfile};
use crate::verification::VERIFY_PROBE;
use crate::wizard_plan::{
    build_wizard_attack_plan, find_profile_mode, rebuild_wizard_plan_from_analysis,
    union_mode_categories, AttackProfileMode, ExecutionStrategy, WizardAttackPlan,
};

const MAX_PROMPT_CHARS: usize = 6_000;
const MAX_REPAIR_JSON_CHARS: usize = 3_000;
const MAX_LLM_ATTEMPTS: u32 = 3;
const PRESET_PROFILE_IDS: [&str; 3] = ["quick", "standard", "deep"];

mod flex {
    use serde::de::{self, Deserializer};
    use serde::Deserialize;
    use serde_json::Value;

    pub fn string<'de, D>(deserializer: D) -> Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Value::deserialize(deserializer)? {
            Value::String(s) => Ok(s),
            Value::Number(n) => Ok(n.to_string()),
            Value::Bool(b) => Ok(b.to_string()),
            Value::Null => Ok(String::new()),
            other => Err(de::Error::custom(format!(
                "expected string-like value, got {other}"
            ))),
        }
    }

    pub fn optional_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Value::deserialize(deserializer)? {
            Value::Null => Ok(None),
            Value::String(s) => Ok(Some(s)),
            Value::Number(n) => Ok(Some(n.to_string())),
            Value::Bool(b) => Ok(Some(b.to_string())),
            other => Err(de::Error::custom(format!(
                "expected string-like value, got {other}"
            ))),
        }
    }

    pub fn optional_string_vec<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Value::deserialize(deserializer)? {
            Value::Null => Ok(None),
            Value::Array(values) => Ok(Some(
                values
                    .into_iter()
                    .map(|value| match value {
                        Value::String(s) => Ok(s),
                        Value::Number(n) => Ok(n.to_string()),
                        other => Err(de::Error::custom(format!(
                            "expected string-like value, got {other}"
                        ))),
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            other => Err(de::Error::custom(format!("expected array, got {other}"))),
        }
    }

    pub fn string_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let values = Vec::<Value>::deserialize(deserializer)?;
        values
            .into_iter()
            .map(|value| match value {
                Value::String(s) => Ok(s),
                Value::Number(n) => Ok(n.to_string()),
                Value::Bool(b) => Ok(b.to_string()),
                other => Err(de::Error::custom(format!(
                    "expected string-like category, got {other}"
                ))),
            })
            .collect()
    }

    pub fn u32_field<'de, D>(deserializer: D) -> Result<u32, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Value::deserialize(deserializer)? {
            Value::Number(n) => n
                .as_u64()
                .and_then(|v| u32::try_from(v).ok())
                .ok_or_else(|| de::Error::custom("invalid u32")),
            Value::String(s) => s.parse().map_err(|_| de::Error::custom("invalid u32 string")),
            Value::Null => Ok(0),
            other => Err(de::Error::custom(format!("expected number, got {other}"))),
        }
    }

    pub fn u8_field<'de, D>(deserializer: D) -> Result<u8, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Value::deserialize(deserializer)? {
            Value::Number(n) => n
                .as_u64()
                .and_then(|v| u8::try_from(v).ok())
                .ok_or_else(|| de::Error::custom("invalid u8")),
            Value::String(s) => s.parse().map_err(|_| de::Error::custom("invalid u8 string")),
            Value::Null => Ok(2),
            other => Err(de::Error::custom(format!("expected number, got {other}"))),
        }
    }

    pub fn optional_u8<'de, D>(deserializer: D) -> Result<Option<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Value::deserialize(deserializer)? {
            Value::Null => Ok(None),
            Value::Number(n) => n
                .as_u64()
                .and_then(|v| u8::try_from(v).ok())
                .map(Some)
                .ok_or_else(|| de::Error::custom("invalid u8")),
            Value::String(s) => s
                .parse()
                .map(Some)
                .map_err(|_| de::Error::custom("invalid u8 string")),
            other => Err(de::Error::custom(format!("expected number, got {other}"))),
        }
    }
}

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
    #[serde(default, deserialize_with = "flex::string")]
    category: String,
    #[serde(default, deserialize_with = "flex::string")]
    reason: String,
    #[serde(default = "default_priority", deserialize_with = "flex::u8_field")]
    priority: u8,
}

#[derive(Debug, Deserialize, Default)]
struct LlmWizardPayloadStrategy {
    #[serde(default, deserialize_with = "flex::string")]
    strategy: String,
    #[serde(default, rename = "mutationLevel", deserialize_with = "flex::string")]
    mutation_level: String,
    #[serde(default, rename = "variantsPerTest", deserialize_with = "flex::u32_field")]
    variants_per_test: u32,
    #[serde(default, rename = "maxTotalPayloads", deserialize_with = "flex::u32_field")]
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
    #[serde(default, deserialize_with = "flex::string_vec")]
    categories: Vec<String>,
    #[serde(default, rename = "executionStrategy", deserialize_with = "flex::string")]
    execution_strategy: String,
    #[serde(default, rename = "maxAttempts", deserialize_with = "flex::optional_u8")]
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
    #[serde(
        default,
        rename = "recommendedProfileId",
        deserialize_with = "flex::optional_string"
    )]
    recommended_profile_id: Option<String>,
    #[serde(default, deserialize_with = "flex::optional_string_vec")]
    disabled_tests: Option<Vec<String>>,
    capabilities: Option<LlmWizardCapabilities>,
    rationales: Option<Vec<LlmWizardRationale>>,
    #[serde(default, deserialize_with = "flex::optional_string")]
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

/// Builds a wizard attack plan from AI Runtime analysis of verify traffic.
/// All three preset modes (quick/standard/deep) MUST come from the LLM response.
pub async fn build_wizard_attack_plan_with_llm(
    profile: &TargetProfile,
    llm: &dyn PlannerLlm,
) -> PlannerResult<WizardAttackPlan> {
    if !profile.is_verified() {
        return Err(PlannerError::Llm(
            "target profile must be verified before AI attack planning".into(),
        ));
    }

    let response_preview = profile
        .verification
        .response_preview
        .as_deref()
        .unwrap_or("")
        .trim();
    if response_preview.is_empty() {
        return Err(PlannerError::Llm(
            "verified response preview is required for AI attack planning".into(),
        ));
    }

    let baseline = build_wizard_attack_plan(profile);
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

    let initial_prompt = aisec_inference::PromptRegistry::wizard_profile_user(
        profile.provider.as_str(),
        &profile.framework,
        &profile.full_url(),
        &format!("{allowed:?}"),
        &format!("{baseline_cats:?}"),
        &truncate_for_prompt(&request_body),
        &truncate_for_prompt(response_preview),
        detected_model,
    );

    let mut last_raw = String::new();
    let mut last_errors = String::new();

    for attempt in 0..MAX_LLM_ATTEMPTS {
        let prompt = if attempt == 0 {
            initial_prompt.clone()
        } else {
            aisec_inference::PromptRegistry::wizard_profile_repair(
                &format!("{allowed:?}"),
                &truncate_for_repair(&last_raw),
                &last_errors,
            )
        };

        let raw = llm.complete(&prompt).await?;
        last_raw = match extract_json_object(&raw) {
            Ok(json) => json,
            Err(err) => {
                last_errors = err.to_string();
                warn!(
                    attempt = attempt + 1,
                    max_attempts = MAX_LLM_ATTEMPTS,
                    error = %last_errors,
                    response_chars = raw.len(),
                    "wizard attack plan: LLM response is not valid JSON"
                );
                continue;
            }
        };

        match parse_wizard_llm_plan(&last_raw, profile) {
            Ok(refinement) => {
                let mut plan = apply_wizard_llm_refinement(profile, baseline, refinement);
                plan.planner_source = "ai_runtime".into();
                return Ok(plan);
            }
            Err(err) => {
                last_errors = enrich_validation_error(&last_raw, &err);
                warn!(
                    attempt = attempt + 1,
                    max_attempts = MAX_LLM_ATTEMPTS,
                    error = %last_errors,
                    json_chars = last_raw.len(),
                    "wizard attack plan: LLM JSON failed validation"
                );
            }
        }
    }

    warn!(
        max_attempts = MAX_LLM_ATTEMPTS,
        last_error = %last_errors,
        "wizard attack plan: all LLM attempts exhausted"
    );

    Err(PlannerError::Llm(format!(
        "AI Runtime failed to produce a valid attack plan after {MAX_LLM_ATTEMPTS} attempts: {last_errors}"
    )))
}

fn truncate_for_prompt(text: &str) -> String {
    if text.len() <= MAX_PROMPT_CHARS {
        return text.to_string();
    }
    format!("{}… [truncated]", &text[..MAX_PROMPT_CHARS])
}

fn truncate_for_repair(text: &str) -> String {
    if text.len() <= MAX_REPAIR_JSON_CHARS {
        return text.to_string();
    }
    format!("{}… [truncated for repair]", &text[..MAX_REPAIR_JSON_CHARS])
}

fn enrich_validation_error(raw: &str, err: &PlannerError) -> String {
    let mut msg = err.to_string();
    if looks_like_truncated_json(raw, &msg) {
        msg.push_str("; response appears truncated — return compact complete JSON with all three modes");
    }
    msg
}

fn looks_like_truncated_json(raw: &str, err_msg: &str) -> bool {
    let lower = err_msg.to_ascii_lowercase();
    if lower.contains("eof while parsing")
        || lower.contains("unterminated json")
        || lower.contains("truncated or unterminated")
    {
        return true;
    }
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') {
        return false;
    }
    let open = trimmed.chars().filter(|c| *c == '{').count();
    let close = trimmed.chars().filter(|c| *c == '}').count();
    open > close
}

fn parse_wizard_llm_plan(raw: &str, profile: &TargetProfile) -> PlannerResult<WizardLlmRefinement> {
    let json_str = extract_json_object(raw)?;
    let parsed: LlmWizardPlanResponse = serde_json::from_str(&json_str)
        .map_err(|e| PlannerError::Llm(format!("invalid JSON: {e}")))?;

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
    let applicable_set: HashSet<_> = applicable.iter().copied().collect();

    let profile_modes = build_modes_from_llm_strict(&parsed.modes, &applicable_set)?;
    validate_profile_modes(&profile_modes, &applicable_set)?;

    let recommended_profile_id = parsed
        .recommended_profile_id
        .as_deref()
        .map(normalize_profile_id)
        .filter(|id| PRESET_PROFILE_IDS.contains(&id.as_str()))
        .ok_or_else(|| {
            PlannerError::Llm("recommendedProfileId must be quick, standard, or deep".into())
        })?;

    if find_profile_mode(&profile_modes, &recommended_profile_id).is_none() {
        return Err(PlannerError::Llm(format!(
            "recommendedProfileId {recommended_profile_id} missing from modes"
        )));
    }

    let suggested_categories = union_mode_categories(&profile_modes);
    if suggested_categories.is_empty() {
        return Err(PlannerError::Llm(
            "modes must include at least one applicable category".into(),
        ));
    }

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

fn build_modes_from_llm_strict(
    modes: &HashMap<String, LlmWizardMode>,
    applicable_set: &HashSet<AttackCategory>,
) -> PlannerResult<Vec<AttackProfileMode>> {
    let mut out = Vec::with_capacity(PRESET_PROFILE_IDS.len());
    for profile_id in PRESET_PROFILE_IDS {
        let llm_mode = modes.get(profile_id).ok_or_else(|| {
            PlannerError::Llm(format!("missing required modes.{profile_id} object"))
        })?;

        let categories = llm_mode
            .categories
            .iter()
            .filter_map(|raw| parse_attack_category(raw))
            .filter(|category| applicable_set.contains(category))
            .collect::<Vec<_>>();
        if categories.is_empty() {
            return Err(PlannerError::Llm(format!(
                "modes.{profile_id}.categories must contain at least one applicable category"
            )));
        }

        let execution_strategy = parse_execution_strategy(&llm_mode.execution_strategy).ok_or_else(
            || {
                PlannerError::Llm(format!(
                    "modes.{profile_id}.executionStrategy must be sequential or agentic"
                ))
            },
        )?;

        let payload_raw = llm_mode.payload_strategy.as_ref().ok_or_else(|| {
            PlannerError::Llm(format!("missing modes.{profile_id}.payloadStrategy"))
        })?;
        let payload_strategy = parse_payload_strategy_strict(payload_raw)?;

        out.push(AttackProfileMode {
            profile_id: (*profile_id).into(),
            categories,
            execution_strategy,
            max_attempts: llm_mode.max_attempts.unwrap_or(5).clamp(1, 20),
            reflection_enabled: llm_mode.reflection_enabled.unwrap_or(false),
            adaptive_planning: llm_mode.adaptive_planning.unwrap_or(false),
            payload_strategy: payload_strategy.clamp(),
        });
    }
    Ok(out)
}

fn validate_profile_modes(
    modes: &[AttackProfileMode],
    applicable_set: &HashSet<AttackCategory>,
) -> PlannerResult<()> {
    let mut errors = Vec::new();
    if modes.len() != PRESET_PROFILE_IDS.len() {
        errors.push(format!(
            "expected {} preset modes, got {}",
            PRESET_PROFILE_IDS.len(),
            modes.len()
        ));
    }
    for profile_id in PRESET_PROFILE_IDS {
        let Some(mode) = find_profile_mode(modes, profile_id) else {
            errors.push(format!("missing modes.{profile_id}"));
            continue;
        };
        if mode.categories.is_empty() {
            errors.push(format!("modes.{profile_id}.categories is empty"));
        }
        if !mode.categories.iter().all(|c| applicable_set.contains(c)) {
            errors.push(format!(
                "modes.{profile_id}.categories contains non-applicable categories"
            ));
        }
        if mode.payload_strategy.max_total_payloads == 0 {
            errors.push(format!(
                "modes.{profile_id}.payloadStrategy.maxTotalPayloads must be >= 1"
            ));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(PlannerError::Llm(errors.join("; ")))
    }
}

fn parse_execution_strategy(raw: &str) -> Option<ExecutionStrategy> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "agentic" | "2" => Some(ExecutionStrategy::Agentic),
        "sequential" | "1" | "0" => Some(ExecutionStrategy::Sequential),
        _ => None,
    }
}

fn parse_payload_strategy_name(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "deterministic" | "1" | "0" => Some("deterministic"),
        "mutation" | "2" => Some("mutation"),
        "adaptive" | "3" => Some("adaptive"),
        _ => None,
    }
}

fn parse_mutation_level_name(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "low" | "1" | "0" => Some("low"),
        "medium" | "2" => Some("medium"),
        "high" | "3" => Some("high"),
        "extreme" | "4" => Some("extreme"),
        _ if raw.is_empty() => Some("medium"),
        _ => None,
    }
}

fn parse_payload_strategy_strict(raw: &LlmWizardPayloadStrategy) -> PlannerResult<PayloadStrategy> {
    let strategy_name = parse_payload_strategy_name(&raw.strategy).ok_or_else(|| {
        PlannerError::Llm(format!(
            "invalid payloadStrategy.strategy: {}",
            raw.strategy
        ))
    })?;
    let strategy = match strategy_name {
        "deterministic" => PayloadGenerationStrategy::Deterministic,
        "adaptive" => PayloadGenerationStrategy::Adaptive,
        "mutation" => PayloadGenerationStrategy::Mutation,
        _ => unreachable!("validated strategy name"),
    };
    let mutation_name = parse_mutation_level_name(&raw.mutation_level).ok_or_else(|| {
        PlannerError::Llm(format!(
            "invalid payloadStrategy.mutationLevel: {}",
            raw.mutation_level
        ))
    })?;
    let mutation_level = match mutation_name {
        "low" => MutationLevel::Low,
        "high" => MutationLevel::High,
        "extreme" => MutationLevel::Extreme,
        "medium" => MutationLevel::Medium,
        other => {
            return Err(PlannerError::Llm(format!(
                "invalid payloadStrategy.mutationLevel: {other}"
            )));
        }
    };
    let variants_per_test = if raw.variants_per_test == 0 {
        default_variants_per_test(strategy)
    } else {
        raw.variants_per_test
    };
    let max_total_payloads = if raw.max_total_payloads == 0 {
        default_max_total_payloads(strategy)
    } else {
        raw.max_total_payloads
    };

    Ok(PayloadStrategy {
        strategy,
        mutation_level,
        variants_per_test,
        max_total_payloads,
        enable_context_awareness: raw.enable_context_awareness,
        enable_conversation_memory: raw.enable_conversation_memory,
        enable_response_adaptation: raw.enable_response_adaptation,
        enable_payload_deduplication: raw.enable_payload_deduplication,
        enable_cross_category_mutation: raw.enable_cross_category_mutation,
    })
}

fn default_max_total_payloads(strategy: PayloadGenerationStrategy) -> u32 {
    match strategy {
        PayloadGenerationStrategy::Deterministic => 10,
        PayloadGenerationStrategy::Mutation => 20,
        PayloadGenerationStrategy::Adaptive => 100,
    }
}

fn default_variants_per_test(strategy: PayloadGenerationStrategy) -> u32 {
    match strategy {
        PayloadGenerationStrategy::Deterministic => 2,
        PayloadGenerationStrategy::Mutation => 5,
        PayloadGenerationStrategy::Adaptive => 10,
    }
}

fn extract_json_object(raw: &str) -> PlannerResult<String> {
    let trimmed = raw.trim();
    let start = trimmed
        .find('{')
        .ok_or_else(|| PlannerError::Llm("no JSON object in LLM response".into()))?;
    let slice = &trimmed[start..];

    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;

    for (idx, ch) in slice.char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            if ch == '\\' {
                escape = true;
                continue;
            }
            if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(slice[..=idx].to_string());
                }
            }
            _ => {}
        }
    }

    Err(PlannerError::Llm(
        "truncated or unterminated JSON in LLM response".into(),
    ))
}

fn apply_wizard_llm_refinement(
    profile: &TargetProfile,
    mut plan: WizardAttackPlan,
    refinement: WizardLlmRefinement,
) -> WizardAttackPlan {
    let mut enhanced = profile.clone();
    if let Some(llm_caps) = refinement.capabilities {
        merge_capabilities_into(&mut enhanced.default_capabilities, &llm_caps);
    }

    let deterministic = plan_from_target_profile(&enhanced);
    let caps = effective_capabilities(&enhanced);
    plan.disabled_tests = refinement.disabled_tests;
    plan.recommended_payload_strategy = recommend_payload_strategy(&enhanced);

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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct MockLlm {
        responses: Vec<String>,
        calls: Arc<AtomicUsize>,
    }

    impl MockLlm {
        fn new(responses: Vec<&str>) -> Self {
            Self {
                responses: responses.into_iter().map(String::from).collect(),
                calls: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    #[async_trait]
    impl PlannerLlm for MockLlm {
        async fn complete(&self, _prompt: &str) -> PlannerResult<String> {
            let idx = self.calls.fetch_add(1, Ordering::SeqCst);
            self.responses
                .get(idx)
                .cloned()
                .ok_or_else(|| PlannerError::Llm("no mock response".into()))
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

    fn valid_modes_json() -> String {
        r#"{
          "recommendedProfileId": "standard",
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
              }
            },
            "deep": {
              "categories": ["prompt_injection", "jailbreak", "tool_abuse"],
              "executionStrategy": "agentic",
              "maxAttempts": 5,
              "payloadStrategy": {
                "strategy": "adaptive",
                "mutationLevel": "extreme",
                "variantsPerTest": 10,
                "maxTotalPayloads": 100
              }
            }
          }
        }"#
        .into()
    }

    #[tokio::test]
    async fn llm_refines_modes_when_runtime_available() {
        let llm = MockLlm::new(vec![valid_modes_json().as_str()]);
        let plan = build_wizard_attack_plan_with_llm(&sample_profile(), &llm)
            .await
            .expect("valid AI plan");
        assert_eq!(plan.profile_modes.len(), 3);
        assert_eq!(plan.planner_source, "ai_runtime");
        assert!(plan.suggested_categories.contains(&AttackCategory::ToolAbuse));
        let standard = find_profile_mode(&plan.profile_modes, "standard").expect("standard mode");
        assert!(standard.categories.contains(&AttackCategory::ToolAbuse));
    }

    #[tokio::test]
    async fn harness_retries_until_modes_are_complete() {
        let incomplete = r#"{"recommendedProfileId":"standard","modes":{"quick":{"categories":["prompt_injection"],"executionStrategy":"sequential","payloadStrategy":{"strategy":"deterministic","mutationLevel":"low","variantsPerTest":2,"maxTotalPayloads":10}}}}"#.to_string();
        let llm = MockLlm::new(vec![incomplete.as_str(), valid_modes_json().as_str()]);
        let plan = build_wizard_attack_plan_with_llm(&sample_profile(), &llm)
            .await
            .expect("repaired AI plan");
        assert_eq!(plan.planner_source, "ai_runtime");
        assert_eq!(plan.profile_modes.len(), 3);
    }

    #[test]
    fn parse_wizard_llm_plan_accepts_numeric_enum_fields() {
        let raw = r#"{
          "recommendedProfileId": "standard",
          "modes": {
            "quick": {
              "categories": ["prompt_injection"],
              "executionStrategy": 1,
              "payloadStrategy": { "strategy": 1 }
            },
            "standard": {
              "categories": ["prompt_injection", "jailbreak"],
              "executionStrategy": "sequential",
              "payloadStrategy": { "strategy": "mutation", "mutationLevel": 2 }
            },
            "deep": {
              "categories": ["prompt_injection", "jailbreak"],
              "executionStrategy": 2,
              "payloadStrategy": { "strategy": 3, "maxTotalPayloads": 100 }
            }
          }
        }"#;
        let profile = sample_profile();
        let refinement = parse_wizard_llm_plan(raw, &profile).expect("numeric enums coerced");
        assert_eq!(refinement.recommended_profile_id, "standard");
        assert_eq!(refinement.profile_modes.len(), 3);
    }

    #[test]
    fn extract_json_object_rejects_truncated_response() {
        let truncated = r#"{"recommendedProfileId":"standard","modes":{"quick":{"categories":["prompt_injection"]"#;
        let err = extract_json_object(truncated).expect_err("truncated JSON");
        assert!(err.to_string().contains("truncated"));
    }

    #[test]
    fn extract_json_object_extracts_balanced_object_from_prose() {
        let raw = r#"Here is the plan:
        {"recommendedProfileId":"standard","modes":{}}
        Thanks."#;
        let json = extract_json_object(raw).expect("balanced JSON");
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
    }

    #[test]
    fn normalize_profile_id_maps_red_team_to_deep() {
        assert_eq!(normalize_profile_id("red_team"), "deep");
        assert_eq!(normalize_profile_id("quick"), "quick");
    }
}
