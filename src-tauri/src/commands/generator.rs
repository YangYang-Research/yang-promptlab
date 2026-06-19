//! Payload generator IPC — build probes from attack plans.

use aisec_attack::AttackCategory;
use aisec_core::AisecError;
use aisec_generator::{generate_from_plan, GeneratorMode, PromptPayloads};
use aisec_judge::{JudgeProviderConfig, JudgeRuntimeContext};
use aisec_judge::providers::LocalLlmBackend;
use aisec_planner::{AttackPlan, PlannerMode};
use aisec_runtime::ModelProviderRuntime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex as AsyncMutex;

use crate::error::{CommandError, CommandResult};
use crate::generator_service::JudgeGeneratorLlm;
use crate::judge_config::{load_judge_config, prepare_judge_runtime_context};
use crate::state::AppState;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorGenerateRequest {
    pub profile_id: String,
    pub categories: Vec<String>,
    pub disabled_tests: Vec<String>,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptPayloadDto {
    pub id: String,
    pub name: String,
    pub category: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorStatsDto {
    pub category_count: usize,
    pub source_count: usize,
    pub payload_count: usize,
    pub variant_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptPayloadsDto {
    pub mode: String,
    pub payloads: Vec<PromptPayloadDto>,
    pub payload_ids: Vec<String>,
    pub stats: GeneratorStatsDto,
    pub summary: String,
    pub llm_note: Option<String>,
}

fn parse_generator_mode(raw: &str) -> GeneratorMode {
    match raw.trim().to_ascii_lowercase().as_str() {
        "template_mutation" | "mutation" | "template" => GeneratorMode::TemplateMutation,
        "local_llm" | "local" | "llm" => GeneratorMode::LocalLlm,
        _ => GeneratorMode::StaticPack,
    }
}

fn parse_categories(raw: &[String]) -> Vec<AttackCategory> {
    raw.iter()
        .filter_map(|value| {
            AttackCategory::all()
                .iter()
                .find(|c| c.as_str() == value.trim())
                .copied()
        })
        .collect()
}

pub fn attack_plan_from_request(request: &GeneratorGenerateRequest) -> CommandResult<AttackPlan> {
    let categories = parse_categories(&request.categories);
    if categories.is_empty() {
        return Err(CommandError::invalid_input(
            "at least one valid attack category is required",
        ));
    }
    Ok(AttackPlan {
        mode: PlannerMode::Deterministic,
        profile_id: request.profile_id.clone(),
        categories,
        disabled_tests: request.disabled_tests.clone(),
        rationales: vec![],
        confidence: 1.0,
        summary: String::new(),
        llm_rationale: None,
    })
}

pub fn payloads_to_dto(pack: PromptPayloads) -> PromptPayloadsDto {
    let payloads: Vec<PromptPayloadDto> = pack
        .by_category
        .values()
        .flat_map(|items| {
            items.iter().map(|p| PromptPayloadDto {
                id: p.id.clone(),
                name: p.name.clone(),
                category: p.category.as_str().into(),
                content: p.content.clone(),
            })
        })
        .collect();

    PromptPayloadsDto {
        mode: match pack.mode {
            GeneratorMode::StaticPack => "static_pack".into(),
            GeneratorMode::TemplateMutation => "template_mutation".into(),
            GeneratorMode::LocalLlm => "local_llm".into(),
        },
        payloads,
        payload_ids: pack.payload_ids,
        stats: GeneratorStatsDto {
            category_count: pack.stats.category_count,
            source_count: pack.stats.source_count,
            payload_count: pack.stats.payload_count,
            variant_count: pack.stats.variant_count,
        },
        summary: pack.summary,
        llm_note: pack.llm_note,
    }
}

pub fn prompt_payloads_map(pack: &PromptPayloads) -> HashMap<AttackCategory, Vec<aisec_attack::AttackPayload>> {
    pack.by_category.clone()
}

pub fn attack_plan_from_scan(
    profile_id: impl Into<String>,
    categories: Vec<AttackCategory>,
    disabled_tests: Vec<String>,
) -> AttackPlan {
    AttackPlan {
        mode: PlannerMode::Deterministic,
        profile_id: profile_id.into(),
        categories,
        disabled_tests,
        rationales: vec![],
        confidence: 1.0,
        summary: String::new(),
        llm_rationale: None,
    }
}

pub fn parse_generator_mode_optional(raw: Option<&str>) -> Option<GeneratorMode> {
    let value = raw?.trim();
    if value.is_empty() {
        return None;
    }
    Some(parse_generator_mode(value))
}

pub async fn generate_payloads_for_scan_job(
    data_dir: &std::path::Path,
    model_manager: &AsyncMutex<aisec_models::LocalModelManager>,
    model_provider: aisec_runtime::SharedModelProvider,
    runtime_supervisor: &AsyncMutex<aisec_runtime::RuntimeSupervisor>,
    plan: &AttackPlan,
    mode: GeneratorMode,
) -> CommandResult<PromptPayloads> {
    if mode == GeneratorMode::LocalLlm {
        let manager = model_manager.lock().await;
        let mut supervisor = runtime_supervisor.lock().await;
        let mut config = load_judge_config(data_dir).await?;
        let runtime = prepare_judge_runtime_context(
            &mut config,
            &manager,
            model_provider,
            &mut supervisor,
        )
        .await?
        .ok_or_else(|| {
            CommandError::invalid_input(
                "local LLM generator requires a vault model — configure Models page first",
            )
        })?;
        drop(manager);
        drop(supervisor);

        let backend = build_generator_llm_backend(&config, &runtime).await?;
        let adapter = JudgeGeneratorLlm::new(backend);
        generate_from_plan(plan, mode, Some(&adapter))
            .await
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))
    } else {
        generate_from_plan(plan, mode, None)
            .await
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))
    }
}

pub async fn generate_payloads_for_plan(
    state: &AppState,
    plan: &AttackPlan,
    mode: GeneratorMode,
) -> CommandResult<PromptPayloads> {
    generate_payloads_for_scan_job(
        state.data_dir(),
        state.model_manager(),
        state.model_provider().clone(),
        state.runtime_supervisor(),
        plan,
        mode,
    )
    .await
}

pub async fn generator_generate_op(
    state: &AppState,
    request: GeneratorGenerateRequest,
) -> CommandResult<PromptPayloadsDto> {
    let plan = attack_plan_from_request(&request)?;
    let mode = parse_generator_mode(&request.mode);
    let pack = generate_payloads_for_plan(state, &plan, mode).await?;
    Ok(payloads_to_dto(pack))
}

async fn build_generator_llm_backend(
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
            "select an active vault model for local LLM generator mode",
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
        Arc::new(AsyncMutex::new(provider_runtime)),
    )))
}

#[tauri::command]
pub async fn generator_generate(
    state: State<'_, AppState>,
    request: GeneratorGenerateRequest,
) -> CommandResult<PromptPayloadsDto> {
    generator_generate_op(state.inner(), request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_static_pack_mode() {
        assert!(matches!(
            parse_generator_mode("static_pack"),
            GeneratorMode::StaticPack
        ));
        assert!(matches!(
            parse_generator_mode("template_mutation"),
            GeneratorMode::TemplateMutation
        ));
    }
}
