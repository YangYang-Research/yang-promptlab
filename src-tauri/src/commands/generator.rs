//! Payload generation helpers for scan execution.

use aisec_attack::{AttackCategory, AttackPayload};
use aisec_core::AisecError;
use aisec_generator::{
    generate_prompt_payloads_with_llm, GeneratePayloadsInput, GeneratorAdvancedOptions,
    GeneratorMode, GeneratorTargetContext, PromptPayloads,
};
use aisec_planner::{AttackPlan, PlannerMode};
use aisec_target_profile::{
    capability_influences_strategy, effective_capabilities, PayloadGenerationStrategy,
    PayloadStrategy, TargetProfile,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;

use crate::error::{CommandError, CommandResult};
use crate::inference_host::{is_inference_ready, HostGeneratorLlm};

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
        // Adaptive uses mutation base; response-adaptation flag drives retry evolution.
        PayloadGenerationStrategy::Adaptive => GeneratorMode::TemplateMutation,
    }
}

pub fn advanced_options_from_strategy(strategy: &PayloadStrategy) -> GeneratorAdvancedOptions {
    GeneratorAdvancedOptions {
        enable_context_awareness: strategy.enable_context_awareness,
        enable_conversation_memory: strategy.enable_conversation_memory,
        enable_response_adaptation: strategy.enable_response_adaptation,
        enable_payload_deduplication: strategy.enable_payload_deduplication,
        enable_cross_category_mutation: strategy.enable_cross_category_mutation,
    }
}

pub fn target_context_from_profile(profile: &TargetProfile) -> GeneratorTargetContext {
    let caps = effective_capabilities(profile);
    let mut capability_notes =
        capability_influences_strategy(&caps, profile.provider.as_str(), &profile.framework);
    if caps.supports_tools {
        capability_notes.push("tools".into());
    }
    if caps.supports_agent {
        capability_notes.push("agent".into());
    }
    if caps.supports_memory {
        capability_notes.push("memory".into());
    }
    if caps.supports_conversation {
        capability_notes.push("conversation".into());
    }
    capability_notes.sort();
    capability_notes.dedup();

    GeneratorTargetContext {
        provider: profile.provider.as_str().into(),
        framework: profile.framework.clone(),
        endpoint: profile.full_url(),
        model: profile.verification.model.clone(),
        capability_notes,
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

        let items = payloads
            .get(category)
            .map(|values| values.as_slice())
            .unwrap_or(&[]);
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

pub fn prompt_payloads_map(pack: &PromptPayloads) -> HashMap<AttackCategory, Vec<AttackPayload>> {
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

pub struct GenerateJobOptions {
    pub mode: GeneratorMode,
    pub max_payloads_per_test: Option<u32>,
    pub advanced: GeneratorAdvancedOptions,
    pub target_context: Option<GeneratorTargetContext>,
    pub adaptation_feedback: Option<String>,
    /// DB-backed catalog; when `None`, uses embedded factory seed.
    pub catalog: Option<aisec_payload::PayloadDatabase>,
}

impl GenerateJobOptions {
    pub fn from_mode(mode: GeneratorMode, max_payloads_per_test: Option<u32>) -> Self {
        Self {
            mode,
            max_payloads_per_test,
            advanced: GeneratorAdvancedOptions::default(),
            target_context: None,
            adaptation_feedback: None,
            catalog: None,
        }
    }

    pub fn from_strategy(
        strategy: &PayloadStrategy,
        profile: Option<&TargetProfile>,
        adaptation_feedback: Option<String>,
    ) -> Self {
        let mut mode = generator_mode_from_payload_strategy(strategy);
        if strategy.enable_response_adaptation
            && adaptation_feedback
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty())
        {
            mode = GeneratorMode::LocalLlm;
        }
        Self {
            mode,
            max_payloads_per_test: Some(strategy.max_total_payloads),
            advanced: advanced_options_from_strategy(strategy),
            target_context: profile.map(target_context_from_profile),
            adaptation_feedback,
            catalog: None,
        }
    }

    pub fn with_catalog(mut self, catalog: aisec_payload::PayloadDatabase) -> Self {
        self.catalog = Some(catalog);
        self
    }
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
    generate_payloads_for_scan_job_with_job_options(
        data_dir,
        inference_manager,
        model_manager,
        model_provider,
        runtime_manager,
        plan,
        GenerateJobOptions::from_mode(mode, max_payloads_per_test),
    )
    .await
}

pub async fn generate_payloads_for_scan_job_with_options_and_catalog(
    data_dir: &std::path::Path,
    inference_manager: Arc<AsyncMutex<aisec_inference::InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<aisec_models::LocalModelManager>>,
    model_provider: aisec_runtime::SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<aisec_runtime::RuntimeManager>>,
    plan: &AttackPlan,
    mode: GeneratorMode,
    max_payloads_per_test: Option<u32>,
    catalog: aisec_payload::PayloadDatabase,
) -> CommandResult<PromptPayloads> {
    generate_payloads_for_scan_job_with_job_options(
        data_dir,
        inference_manager,
        model_manager,
        model_provider,
        runtime_manager,
        plan,
        GenerateJobOptions::from_mode(mode, max_payloads_per_test).with_catalog(catalog),
    )
    .await
}

pub async fn generate_payloads_for_scan_job_with_job_options(
    data_dir: &std::path::Path,
    inference_manager: Arc<AsyncMutex<aisec_inference::InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<aisec_models::LocalModelManager>>,
    model_provider: aisec_runtime::SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<aisec_runtime::RuntimeManager>>,
    plan: &AttackPlan,
    mut options: GenerateJobOptions,
) -> CommandResult<PromptPayloads> {
    if options.mode == GeneratorMode::LocalLlm {
        let inference = inference_manager.lock().await;
        if !is_inference_ready(&inference) {
            if options.adaptation_feedback.is_some() {
                options.mode = GeneratorMode::TemplateMutation;
            } else {
                return Err(CommandError::invalid_input(
                    "AI runtime is not configured for local LLM generation",
                ));
            }
        }
    }

    let catalog_owned = options
        .catalog
        .take()
        .or_else(|| aisec_payload::PayloadDatabase::builtin().ok());
    let catalog_ref = catalog_owned.as_ref();

    let input = GeneratePayloadsInput {
        plan,
        mode: options.mode,
        max_payloads_per_test: options.max_payloads_per_test,
        advanced: options.advanced.clone(),
        target_context: options.target_context.clone(),
        adaptation_feedback: options.adaptation_feedback.clone(),
        catalog: catalog_ref,
    };

    let pack = if options.mode == GeneratorMode::LocalLlm {
        let llm = Arc::new(HostGeneratorLlm::new(
            data_dir.to_path_buf(),
            Arc::clone(&inference_manager),
            Arc::clone(&model_manager),
            model_provider,
            Arc::clone(&runtime_manager),
        ));
        generate_prompt_payloads_with_llm(&input, Some(llm.as_ref()))
            .await
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?
    } else {
        aisec_generator::generate_prompt_payloads(&input)
            .await
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?
    };

    Ok(if let Some(max_per_test) = options.max_payloads_per_test {
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
    generate_payloads_for_scan_job_with_strategy_context(
        data_dir,
        inference_manager,
        model_manager,
        model_provider,
        runtime_manager,
        plan,
        strategy,
        None,
        None,
    )
    .await
}

pub async fn generate_payloads_for_scan_job_with_strategy_context(
    data_dir: &std::path::Path,
    inference_manager: Arc<AsyncMutex<aisec_inference::InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<aisec_models::LocalModelManager>>,
    model_provider: aisec_runtime::SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<aisec_runtime::RuntimeManager>>,
    plan: &AttackPlan,
    strategy: &PayloadStrategy,
    profile: Option<&TargetProfile>,
    adaptation_feedback: Option<String>,
) -> CommandResult<PromptPayloads> {
    generate_payloads_for_scan_job_with_strategy_context_and_catalog(
        data_dir,
        inference_manager,
        model_manager,
        model_provider,
        runtime_manager,
        plan,
        strategy,
        profile,
        adaptation_feedback,
        None,
    )
    .await
}

pub async fn generate_payloads_for_scan_job_with_strategy_context_and_catalog(
    data_dir: &std::path::Path,
    inference_manager: Arc<AsyncMutex<aisec_inference::InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<aisec_models::LocalModelManager>>,
    model_provider: aisec_runtime::SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<aisec_runtime::RuntimeManager>>,
    plan: &AttackPlan,
    strategy: &PayloadStrategy,
    profile: Option<&TargetProfile>,
    adaptation_feedback: Option<String>,
    catalog: Option<aisec_payload::PayloadDatabase>,
) -> CommandResult<PromptPayloads> {
    let mut options = GenerateJobOptions::from_strategy(strategy, profile, adaptation_feedback);
    if let Some(catalog) = catalog {
        options = options.with_catalog(catalog);
    }
    generate_payloads_for_scan_job_with_job_options(
        data_dir,
        inference_manager,
        model_manager,
        model_provider,
        runtime_manager,
        plan,
        options,
    )
    .await
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

    #[test]
    fn advanced_options_map_from_strategy() {
        let strategy = PayloadStrategy {
            enable_context_awareness: true,
            enable_payload_deduplication: true,
            enable_cross_category_mutation: true,
            ..PayloadStrategy::default()
        };
        let advanced = advanced_options_from_strategy(&strategy);
        assert!(advanced.enable_context_awareness);
        assert!(advanced.enable_payload_deduplication);
        assert!(advanced.enable_cross_category_mutation);
        assert!(!advanced.enable_conversation_memory);
    }
}
