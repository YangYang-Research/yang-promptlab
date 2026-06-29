//! Attack planner IPC — generate scan plans from endpoint fingerprints.

use aisec_core::AisecError;
use aisec_attack::AttackCategory;
use aisec_fingerprint::StackFingerprintReport;
use aisec_planner::{
    generate_attack_plan, FingerprintEndpoint, FingerprintResult, PlannerMode,
};
use aisec_storage::TargetRepository;
use aisec_target_profile::{adjust_wizard_attack_plan, build_wizard_attack_plan, summary_for_api_endpoint, ExecutionStrategy, WizardAttackPlan};
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
pub struct AttackGraphNodeDto {
    pub category: String,
    pub priority: u8,
    pub risk: u8,
    pub confidence: f32,
    pub dependencies: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WizardAttackPlanDto {
    pub profile_id: String,
    pub suggested_categories: Vec<String>,
    pub categories: Vec<String>,
    pub disabled_tests: Vec<String>,
    pub capability_graph: Vec<String>,
    pub attack_graph: Vec<AttackGraphNodeDto>,
    pub execution_strategy: String,
    pub max_attempts: u8,
    pub reflection_enabled: bool,
    pub adaptive_planning: bool,
    pub rationales: Vec<CategoryRationaleDto>,
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
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannerAdjustRequest {
    pub target_id: String,
    pub profile_id: String,
    pub categories: Vec<String>,
    pub disabled_tests: Vec<String>,
    pub disabled_graph_nodes: Vec<String>,
    pub execution_strategy: Option<String>,
    pub max_attempts: Option<u8>,
    pub reflection_enabled: Option<bool>,
    pub adaptive_planning: Option<bool>,
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

pub fn wizard_plan_to_dto(plan: WizardAttackPlan) -> WizardAttackPlanDto {
    WizardAttackPlanDto {
        profile_id: plan.profile_id,
        suggested_categories: plan
            .suggested_categories
            .iter()
            .map(|c| c.as_str().into())
            .collect(),
        categories: plan.categories.iter().map(|c| c.as_str().into()).collect(),
        disabled_tests: plan.disabled_tests,
        capability_graph: plan.capability_graph,
        attack_graph: plan
            .attack_graph
            .into_iter()
            .map(|node| AttackGraphNodeDto {
                category: node.category.as_str().into(),
                priority: node.priority,
                risk: node.risk,
                confidence: node.confidence,
                dependencies: node
                    .dependencies
                    .iter()
                    .map(|c| c.as_str().into())
                    .collect(),
                enabled: node.enabled,
            })
            .collect(),
        execution_strategy: match plan.execution_strategy {
            ExecutionStrategy::Sequential => "sequential".into(),
            ExecutionStrategy::Agentic => "agentic".into(),
        },
        max_attempts: plan.max_attempts,
        reflection_enabled: plan.reflection_enabled,
        adaptive_planning: plan.adaptive_planning,
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
        risk_score: plan.risk_score,
        risk_level: plan.risk_level,
        estimated_requests: plan.estimated_requests,
        estimated_runtime_seconds: plan.estimated_runtime_seconds,
        estimated_tokens: plan.estimated_tokens,
        coverage_score: plan.coverage_score,
        risk_coverage: plan.risk_coverage,
        total_testcases: plan.total_testcases,
    }
}

fn parse_attack_categories(raw: &[String]) -> Vec<AttackCategory> {
    raw.iter()
        .filter_map(|value| AttackCategory::all().iter().copied().find(|c| c.as_str() == value))
        .collect()
}

fn parse_execution_strategy(raw: Option<&str>) -> ExecutionStrategy {
    match raw.unwrap_or("sequential").trim().to_ascii_lowercase().as_str() {
        "agentic" => ExecutionStrategy::Agentic,
        _ => ExecutionStrategy::Sequential,
    }
}

pub async fn planner_adjust_wizard_plan_op(
    state: &AppState,
    request: PlannerAdjustRequest,
) -> CommandResult<WizardAttackPlanDto> {
    let target = state
        .repositories()
        .targets()
        .get(&request.target_id)
        .await
        .map_err(CommandError::from)?;
    let profile = crate::commands::target_profile::parse_target_profile(&target.profile_json)?;
    if !profile.is_verified() {
        return Err(CommandError::invalid_input(
            "Target profile must be verified before adjusting attack plan",
        ));
    }

    let base = build_wizard_attack_plan(&profile);
    let categories = if request.profile_id.eq_ignore_ascii_case("custom") {
        Some(parse_attack_categories(&request.categories))
    } else {
        None
    };
    let mut plan = adjust_wizard_attack_plan(
        base,
        &request.profile_id,
        categories,
        &request.disabled_tests,
        &request.disabled_graph_nodes,
    );
    plan.execution_strategy = parse_execution_strategy(request.execution_strategy.as_deref());
    if let Some(max) = request.max_attempts {
        plan.max_attempts = max.clamp(1, 20);
    }
    if let Some(enabled) = request.reflection_enabled {
        plan.reflection_enabled = enabled;
    }
    if let Some(enabled) = request.adaptive_planning {
        plan.adaptive_planning = enabled;
    }
    plan.summary = summary_for_api_endpoint(&profile.full_url(), plan.categories.len());
    Ok(wizard_plan_to_dto(plan))
}

pub fn plan_to_dto(plan: aisec_planner::AttackPlan) -> AttackPlanDto {
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
        let report = crate::dto::stack_fingerprint_from_endpoint(&endpoint).ok_or_else(|| {
            CommandError::invalid_input(format!(
                "endpoint {} has no AI metadata — re-run discovery first",
                endpoint.url
            ))
        })?;
        let metadata = crate::dto::metadata_from_endpoint(&endpoint).ok_or_else(|| {
            CommandError::invalid_input(format!(
                "endpoint {} metadata is invalid — re-run discovery first",
                endpoint.url
            ))
        })?;
        endpoints.push(FingerprintEndpoint {
            endpoint_id: endpoint.id,
            url: endpoint.url,
            report,
            metadata: Some(metadata),
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
pub async fn attack_planner_adjust(
    state: State<'_, AppState>,
    request: PlannerAdjustRequest,
) -> CommandResult<WizardAttackPlanDto> {
    planner_adjust_wizard_plan_op(state.inner(), request).await
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
    use aisec_attack::AttackCategory;
    use aisec_planner::PlannerMode;

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
