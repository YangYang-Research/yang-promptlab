//! Payload generator IPC — build probes from attack plans.

use aisec_attack::{AttackCategory, AttackPayload};
use aisec_core::AisecError;
use aisec_generator::{generate_from_plan, GeneratePayloadsInput, GeneratorMode, PromptPayloads};
use aisec_planner::{AttackPlan, PlannerMode};
use aisec_target_profile::{PayloadGenerationStrategy, PayloadStrategy};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex as AsyncMutex;

use crate::inference_host::{is_inference_ready, HostGeneratorLlm};
use crate::error::{CommandError, CommandResult};
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
        "deterministic" | "static_pack" | "static" => GeneratorMode::StaticPack,
        "adaptive" => GeneratorMode::TemplateMutation,
        _ => GeneratorMode::StaticPack,
    }
}

pub fn generator_mode_from_payload_strategy(strategy: &PayloadStrategy) -> GeneratorMode {
    match strategy.strategy {
        PayloadGenerationStrategy::Deterministic => GeneratorMode::StaticPack,
        PayloadGenerationStrategy::Mutation => GeneratorMode::TemplateMutation,
        PayloadGenerationStrategy::Adaptive => GeneratorMode::TemplateMutation,
    }
}

fn payload_source_key(payload: &aisec_attack::AttackPayload) -> String {
    payload
        .metadata
        .get("source_id")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            payload
                .id
                .split(':')
                .next()
                .unwrap_or(payload.id.as_str())
                .to_string()
        })
}

/// Cap generated payloads per testcase source within each category.
pub fn cap_payloads_per_testcase(mut pack: PromptPayloads, max_per_test: u32) -> PromptPayloads {
    let limit = max_per_test as usize;
    if limit == 0 {
        return pack;
    }

    let mut kept = 0usize;
    let mut capped_map = HashMap::new();
    for (category, items) in pack.by_category {
        let mut per_source: HashMap<String, Vec<aisec_attack::AttackPayload>> = HashMap::new();
        for item in items {
            let key = payload_source_key(&item);
            let bucket = per_source.entry(key).or_default();
            if bucket.len() < limit {
                bucket.push(item);
            }
        }
        let capped: Vec<_> = per_source.into_values().flatten().collect();
        kept += capped.len();
        if !capped.is_empty() {
            capped_map.insert(category, capped);
        }
    }

    pack.by_category = capped_map;
    pack.stats.payload_count = kept;
    pack.payload_ids = pack
        .by_category
        .values()
        .flat_map(|items| items.iter().map(|p| p.id.clone()))
        .collect();
    pack
}

pub fn validate_payload_budget(
    pack: &PromptPayloads,
    categories: &[AttackCategory],
    disabled_tests: &[String],
    strategy: &PayloadStrategy,
) -> Result<(), String> {
    validate_payload_map_per_testcase(&pack.by_category, categories, disabled_tests, strategy)
}

pub fn validate_payload_map_budget(
    payloads: &HashMap<AttackCategory, Vec<AttackPayload>>,
    categories: &[AttackCategory],
    disabled_tests: &[String],
    strategy: &PayloadStrategy,
) -> Result<(), String> {
    validate_payload_map_per_testcase(payloads, categories, disabled_tests, strategy)
}

pub fn validate_payload_map_per_testcase(
    payloads: &HashMap<AttackCategory, Vec<AttackPayload>>,
    categories: &[AttackCategory],
    disabled_tests: &[String],
    strategy: &PayloadStrategy,
) -> Result<(), String> {
    let budget = strategy.max_total_payloads;
    if budget == 0 {
        return Ok(());
    }

    for category in categories {
        let expected_tests =
            aisec_target_profile::wizard_plan::enabled_tests_for_category(*category, disabled_tests);
        if expected_tests == 0 {
            continue;
        }

        let items = payloads.get(category).map(|values| values.as_slice()).unwrap_or(&[]);
        let mut per_source: HashMap<String, u32> = HashMap::new();
        for item in items {
            *per_source.entry(payload_source_key(item)).or_insert(0) += 1;
        }

        if per_source.len() < expected_tests as usize {
            return Err(format!(
                "category {} produced {} testcase payloads but {} enabled tests require coverage",
                category.as_str(),
                per_source.len(),
                expected_tests
            ));
        }

        for (source_id, count) in &per_source {
            if *count < budget {
                return Err(format!(
                    "testcase {source_id} in {} has {count} payloads but {budget} required per testcase",
                    category.as_str()
                ));
            }
        }
    }

    Ok(())
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
    inference_manager: Arc<AsyncMutex<aisec_inference::InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<aisec_models::LocalModelManager>>,
    model_provider: aisec_runtime::SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<aisec_runtime::RuntimeManager>>,
    plan: &AttackPlan,
    mode: GeneratorMode,
) -> CommandResult<PromptPayloads> {
    generate_payloads_for_scan_job_with_options(
        data_dir,
        inference_manager,
        model_manager,
        model_provider,
        runtime_manager,
        plan,
        mode,
        None,
    )
    .await
}

pub async fn generate_payloads_for_scan_job_with_options(
    data_dir: &std::path::Path,
    inference_manager: Arc<AsyncMutex<aisec_inference::InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<aisec_models::LocalModelManager>>,
    model_provider: aisec_runtime::SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<aisec_runtime::RuntimeManager>>,
    plan: &AttackPlan,
    mode: GeneratorMode,
    max_payloads_per_test: Option<u32>,
) -> CommandResult<PromptPayloads> {
    let input = GeneratePayloadsInput {
        plan,
        mode,
        max_payloads_per_test,
    };

    let pack = if mode == GeneratorMode::LocalLlm {
        let inference = inference_manager.lock().await;
        if !is_inference_ready(&inference) {
            return Err(CommandError::invalid_input(
                "AI runtime is not configured for local LLM generation",
            ));
        }
        drop(inference);
        let llm = Arc::new(HostGeneratorLlm::new(
            data_dir.to_path_buf(),
            Arc::clone(&inference_manager),
            Arc::clone(&model_manager),
            model_provider,
            Arc::clone(&runtime_manager),
        ));
        generate_from_plan(plan, mode, Some(llm.as_ref()))
            .await
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?
    } else {
        aisec_generator::generate_prompt_payloads(&input)
            .await
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?
    };

    Ok(if let Some(max_per_test) = max_payloads_per_test {
        cap_payloads_per_testcase(pack, max_per_test)
    } else {
        pack
    })
}

pub async fn generate_payloads_for_scan_job_with_strategy(
    data_dir: &std::path::Path,
    inference_manager: Arc<AsyncMutex<aisec_inference::InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<aisec_models::LocalModelManager>>,
    model_provider: aisec_runtime::SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<aisec_runtime::RuntimeManager>>,
    plan: &AttackPlan,
    strategy: &PayloadStrategy,
) -> CommandResult<PromptPayloads> {
    let mode = generator_mode_from_payload_strategy(strategy);
    generate_payloads_for_scan_job_with_options(
        data_dir,
        inference_manager,
        model_manager,
        model_provider,
        runtime_manager,
        plan,
        mode,
        Some(strategy.max_total_payloads),
    )
    .await
}

pub async fn generate_payloads_for_plan(
    state: &AppState,
    plan: &AttackPlan,
    mode: GeneratorMode,
) -> CommandResult<PromptPayloads> {
    generate_payloads_for_scan_job(
        state.data_dir(),
        Arc::clone(state.inference_manager()),
        Arc::clone(state.model_manager()),
        state.model_provider().clone(),
        Arc::clone(state.runtime_manager()),
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
