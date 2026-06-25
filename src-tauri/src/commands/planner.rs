//! Attack planner IPC — generate scan plans from endpoint fingerprints.

use aisec_core::AisecError;
use aisec_fingerprint::StackFingerprintReport;
use aisec_planner::{
    generate_attack_plan, FingerprintEndpoint, FingerprintResult, PlannerMode,
};
use aisec_storage::EndpointRepository;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

use crate::inference_host::{is_inference_ready, HostPlannerLlm};
use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannerGenerateRequest {
    pub endpoint_ids: Vec<String>,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttackPlanDto {
    pub mode: String,
    pub profile_id: String,
    pub categories: Vec<String>,
    pub disabled_tests: Vec<String>,
    pub rationales: Vec<CategoryRationaleDto>,
    pub confidence: f32,
    pub summary: String,
    pub llm_rationale: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRationaleDto {
    pub category: String,
    pub reason: String,
    pub priority: u8,
    pub source: String,
}

fn parse_planner_mode(raw: &str) -> PlannerMode {
    match raw.trim().to_ascii_lowercase().as_str() {
        "local_llm" | "local" | "llm" => PlannerMode::LocalLlm,
        _ => PlannerMode::Deterministic,
    }
}

fn plan_to_dto(plan: aisec_planner::AttackPlan) -> AttackPlanDto {
    AttackPlanDto {
        mode: match plan.mode {
            PlannerMode::Deterministic => "deterministic".into(),
            PlannerMode::LocalLlm => "local_llm".into(),
        },
        profile_id: plan.profile_id,
        categories: plan.categories.iter().map(|c| c.as_str().into()).collect(),
        disabled_tests: plan.disabled_tests,
        rationales: plan
            .rationales
            .into_iter()
            .map(|r| CategoryRationaleDto {
                category: r.category.as_str().into(),
                reason: r.reason,
                priority: r.priority,
                source: r.source,
            })
            .collect(),
        confidence: plan.confidence,
        summary: plan.summary,
        llm_rationale: plan.llm_rationale,
    }
}

pub async fn planner_generate_op(
    state: &AppState,
    request: PlannerGenerateRequest,
) -> CommandResult<AttackPlanDto> {
    if request.endpoint_ids.is_empty() {
        return Err(CommandError::invalid_input(
            "select at least one endpoint for attack planning",
        ));
    }

    let repos = state.repositories();
    let mut endpoints = Vec::new();
    for id in &request.endpoint_ids {
        let endpoint = repos
            .endpoints()
            .get(id)
            .await
            .map_err(CommandError::from)?;
        let report: StackFingerprintReport = endpoint
            .fingerprint_json
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok())
            .ok_or_else(|| {
                CommandError::invalid_input(format!(
                    "endpoint {} has no fingerprint — re-run discovery first",
                    endpoint.url
                ))
            })?;
        endpoints.push(FingerprintEndpoint {
            endpoint_id: endpoint.id,
            url: endpoint.url,
            report,
        });
    }

    let input = FingerprintResult { endpoints };
    let mode = parse_planner_mode(&request.mode);

    let plan = if mode == PlannerMode::LocalLlm {
        let inference = state.inference_manager().lock().await;
        if !is_inference_ready(&inference) {
            return Err(CommandError::invalid_input(
                "AI runtime is not configured for local LLM planning",
            ));
        }
        drop(inference);
        let llm = Arc::new(HostPlannerLlm::new(
            state.data_dir().to_path_buf(),
            state.inference_manager().clone(),
            state.model_manager().clone(),
            state.model_provider().clone(),
            state.runtime_manager().clone(),
        ));
        generate_attack_plan(&input, mode, Some(llm.as_ref()))
            .await
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?
    } else {
        generate_attack_plan(&input, mode, None)
            .await
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?
    };

    Ok(plan_to_dto(plan))
}

#[tauri::command]
pub async fn planner_generate(
    state: State<'_, AppState>,
    request: PlannerGenerateRequest,
) -> CommandResult<AttackPlanDto> {
    planner_generate_op(state.inner(), request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_attack_categories_to_strings() {
        let dto = plan_to_dto(aisec_planner::AttackPlan {
            mode: PlannerMode::Deterministic,
            profile_id: "custom".into(),
            categories: vec![
                AttackCategory::PromptInjection,
                AttackCategory::ToolAbuse,
            ],
            disabled_tests: vec![],
            rationales: vec![],
            confidence: 0.8,
            summary: "test".into(),
            llm_rationale: None,
        });
        assert_eq!(dto.categories, vec!["prompt_injection", "tool_abuse"]);
    }
}
