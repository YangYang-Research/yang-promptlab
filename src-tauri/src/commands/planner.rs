//! Attack planner IPC — generate scan plans from endpoint fingerprints.

use aisec_attack::AttackCategory;
use aisec_core::AisecError;
use aisec_fingerprint::StackFingerprintReport;
use aisec_judge::{JudgeProviderConfig, JudgeRuntimeContext};
use aisec_judge::providers::LocalLlmBackend;
use aisec_planner::{
    generate_attack_plan, FingerprintEndpoint, FingerprintResult, PlannerMode,
};
use aisec_runtime::ModelProviderRuntime;
use aisec_storage::EndpointRepository;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

use crate::error::{CommandError, CommandResult};
use crate::judge_config::{load_judge_config, prepare_judge_runtime_context};
use crate::planner_service::JudgePlannerLlm;
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
        let mut config = load_judge_config(state.data_dir()).await?;
        let manager = state.model_manager().lock().await;
        let mut supervisor = state.runtime_supervisor().lock().await;
        let runtime = prepare_judge_runtime_context(
            &mut config,
            &manager,
            state.model_provider().clone(),
            &mut supervisor,
        )
        .await?
        .ok_or_else(|| {
            CommandError::invalid_input(
                "local LLM planner requires a vault model — configure Models page first",
            )
        })?;
        drop(manager);
        drop(supervisor);

        let backend = build_planner_llm_backend(&config, &runtime).await?;
        let adapter = JudgePlannerLlm::new(backend);
        generate_attack_plan(&input, mode, Some(&adapter))
            .await
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?
    } else {
        generate_attack_plan(&input, mode, None)
            .await
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?
    };

    Ok(plan_to_dto(plan))
}

async fn build_planner_llm_backend(
    config: &JudgeProviderConfig,
    runtime: &JudgeRuntimeContext,
) -> CommandResult<Arc<dyn aisec_judge::providers::LlmBackend>> {
    let model_id = config
        .local
        .vault_model_id
        .clone()
        .unwrap_or_else(|| runtime.active_model_id.clone());
    if model_id.trim().is_empty() {
        return Err(CommandError::invalid_input(
            "select an active vault model for local LLM planner mode",
        ));
    }

    let provider_runtime =
        ModelProviderRuntime::new(runtime.model_provider.clone(), model_id.clone());
    let label = match config.local.provider {
        aisec_judge::LocalProvider::Ollama => "runtime/ollama",
        aisec_judge::LocalProvider::LlamaCpp => "runtime/llama_cpp",
    };

    Ok(Arc::new(LocalLlmBackend::new(
        label,
        config.local.model.clone(),
        Arc::new(Mutex::new(provider_runtime)),
    )))
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
