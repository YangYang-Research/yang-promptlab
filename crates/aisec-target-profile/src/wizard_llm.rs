//! LLM-assisted wizard attack planning from verified API request/response.

use aisec_attack::AttackCategory;
use aisec_planner::types::CategoryRationale;
use aisec_planner::{parse_attack_category, PlannerLlm, PlannerResult};
use serde::Deserialize;

use crate::capabilities::{effective_capabilities, merge_capabilities_into};
use crate::planner::plan_from_target_profile;
use crate::prompt::replace_prompt;
use crate::types::{TargetCapabilities, TargetProfile};
use crate::verification::VERIFY_PROBE;
use crate::wizard_plan::{
    build_wizard_attack_plan, rebuild_wizard_plan_from_analysis, WizardAttackPlan,
};

const MAX_PROMPT_CHARS: usize = 6_000;

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

fn default_priority() -> u8 {
    2
}

#[derive(Debug, Deserialize)]
struct LlmWizardPlanResponse {
    profile_id: Option<String>,
    categories: Vec<String>,
    disabled_tests: Option<Vec<String>>,
    capabilities: Option<LlmWizardCapabilities>,
    rationales: Option<Vec<LlmWizardRationale>>,
    rationale: Option<String>,
}

struct WizardLlmRefinement {
    profile_id: String,
    categories: Vec<AttackCategory>,
    disabled_tests: Vec<String>,
    rationales: Vec<CategoryRationale>,
    capabilities: Option<TargetCapabilities>,
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
        Ok(raw) => match parse_wizard_llm_plan(&raw, &baseline) {
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
    baseline: &WizardAttackPlan,
) -> PlannerResult<WizardLlmRefinement> {
    let json_str = extract_json_object(raw)?;
    let parsed: LlmWizardPlanResponse = serde_json::from_str(&json_str)
        .map_err(|e| aisec_planner::PlannerError::Llm(format!("invalid JSON: {e}")))?;

    let mut categories = Vec::new();
    for raw_cat in parsed.categories {
        if let Some(category) = parse_attack_category(&raw_cat) {
            categories.push(category);
        }
    }
    if categories.is_empty() {
        categories = baseline.suggested_categories.clone();
    }

    let profile_id = parsed
        .profile_id
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| baseline.profile_id.clone());

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
        if let Some(summary) = parsed.rationale.filter(|s| !s.is_empty()) {
            for category in &categories {
                rationales.push(CategoryRationale {
                    category: *category,
                    reason: summary.clone(),
                    priority: 2,
                    source: "ai_runtime".into(),
                });
            }
        }
    }

    let capabilities = parsed.capabilities.map(|caps| TargetCapabilities {
        supports_streaming: caps.supports_streaming,
        supports_tools: caps.supports_tools,
        supports_conversation: caps.supports_conversation,
        supports_attachments: caps.supports_attachments,
        supports_memory: caps.supports_memory,
        supports_agent: caps.supports_agent,
    });

    Ok(WizardLlmRefinement {
        profile_id,
        categories,
        disabled_tests: parsed.disabled_tests.unwrap_or_default(),
        rationales,
        capabilities,
    })
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
    refinement: WizardLlmRefinement,
) -> WizardAttackPlan {
    let mut enhanced = profile.clone();
    if let Some(llm_caps) = refinement.capabilities {
        merge_capabilities_into(&mut enhanced.default_capabilities, &llm_caps);
    }

    let deterministic = plan_from_target_profile(&enhanced);
    let suggested = if refinement.categories.is_empty() {
        deterministic.categories.clone()
    } else {
        refinement.categories
    };

    let rationales = if refinement.rationales.is_empty() {
        deterministic.rationales
    } else {
        refinement.rationales
    };

    let caps = effective_capabilities(&enhanced);
    let profile_id = normalize_profile_id(&refinement.profile_id);
    plan.disabled_tests = refinement.disabled_tests;

    rebuild_wizard_plan_from_analysis(
        profile,
        plan,
        profile_id,
        suggested,
        rationales,
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
    async fn llm_refines_categories_when_runtime_available() {
        let llm_json = r#"{
          "profile_id": "standard",
          "categories": ["prompt_injection", "jailbreak", "tool_abuse"],
          "capabilities": {
            "supportsTools": true,
            "supportsConversation": true,
            "supportsMemory": true,
            "supportsAgent": true
          },
          "rationales": [
            { "category": "tool_abuse", "reason": "tools in request schema", "priority": 1 }
          ]
        }"#;

        let plan =
            build_wizard_attack_plan_with_llm(&sample_profile(), Some(&MockLlm(llm_json))).await;
        assert!(plan.suggested_categories.contains(&AttackCategory::ToolAbuse));
        assert!(plan.rationales.iter().any(|r| r.source == "ai_runtime"));
    }

    #[test]
    fn normalize_profile_id_maps_red_team_to_deep() {
        assert_eq!(normalize_profile_id("red_team"), "deep");
        assert_eq!(normalize_profile_id("quick"), "quick");
    }
}
