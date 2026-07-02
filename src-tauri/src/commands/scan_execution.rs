//! Scan attack orchestration — payload preparation and sequential/agentic flows.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aisec_attack::{AttackCategory, AttackPayload};
use aisec_generator::GeneratorMode;
use aisec_inference::InferenceRuntimeManager;
use aisec_models::LocalModelManager;
use aisec_planner::AttackPlan;
use aisec_runtime::{RuntimeManager, SharedModelProvider};
use aisec_storage::Repositories;
use aisec_target_profile::PayloadStrategy;
use aisec_target_profile::wizard_plan::{estimate_scan_requests, ExecutionStrategy};
use tauri::async_runtime::Mutex as AsyncMutex;
use tracing::{info, warn};

use crate::commands::attack::{CategoryRunOptions, CategoryRunResult, run_category_on_target_profile};
use crate::commands::generator::{
    attack_plan_from_scan, generate_payloads_for_scan_job_with_options,
    generate_payloads_for_scan_job_with_strategy, generator_mode_from_payload_strategy,
    parse_generator_mode_optional, prompt_payloads_map, validate_payload_map_budget,
};
use crate::events::ScanProgressEmitter;
use crate::jobs::ScanProgress;
use crate::session_auth::AttackRuntime;

pub struct ScanExecutionConfig {
    pub execution: ExecutionStrategy,
    pub max_attempts: u32,
    pub reflection_enabled: bool,
    pub payload_strategy: Option<PayloadStrategy>,
    pub generator_mode: Option<String>,
    /// Delay before payload generation so the wizard attack screen can render.
    pub pipeline_warmup_secs: u32,
}

impl ScanExecutionConfig {
    pub fn from_flags(
        agentic: bool,
        max_attempts: usize,
        reflection_enabled: bool,
        payload_strategy: Option<PayloadStrategy>,
        generator_mode: Option<String>,
    ) -> Self {
        Self {
            execution: if agentic {
                ExecutionStrategy::Agentic
            } else {
                ExecutionStrategy::Sequential
            },
            max_attempts: max_attempts.max(1) as u32,
            reflection_enabled,
            payload_strategy,
            generator_mode,
            pipeline_warmup_secs: 3,
        }
    }
}

pub fn scan_progress_total(
    categories: &[AttackCategory],
    disabled_tests: &[String],
    config: &ScanExecutionConfig,
) -> u64 {
    let strategy = config.payload_strategy.clone().unwrap_or_default();
    estimate_scan_requests(
        categories,
        disabled_tests,
        &strategy,
        config.execution,
        config.max_attempts,
    )
    .max(1) as u64
}

fn set_scan_phase(
    progress: &Arc<Mutex<ScanProgress>>,
    phase: &str,
    test: Option<&str>,
    attempt: Option<u32>,
    retry: Option<u32>,
) {
    if let Ok(mut state) = progress.lock() {
        state.current_phase = Some(phase.into());
        if let Some(label) = test {
            state.current_test = Some(label.into());
        }
        state.current_attempt = attempt;
        state.current_retry = retry;
    }
}

fn escalated_generator_mode(strategy: &PayloadStrategy, retry: u32) -> GeneratorMode {
    match retry {
        0 => generator_mode_from_payload_strategy(strategy),
        1 => GeneratorMode::TemplateMutation,
        _ => GeneratorMode::LocalLlm,
    }
}

fn category_payload_map(
    all: &HashMap<AttackCategory, Vec<AttackPayload>>,
    category: AttackCategory,
) -> HashMap<AttackCategory, Vec<AttackPayload>> {
    let mut map = HashMap::new();
    if let Some(items) = all.get(&category) {
        map.insert(category, items.clone());
    }
    map
}

fn category_any_vulnerable(result: &CategoryRunResult) -> bool {
    result.judged.iter().any(|item| item.vulnerable)
}

fn reflection_allows_retry(
    reflection_enabled: bool,
    result: &CategoryRunResult,
    emitter: &ScanProgressEmitter,
) -> bool {
    if !reflection_enabled {
        return !category_any_vulnerable(result);
    }
    let vulnerable = category_any_vulnerable(result);
    let high_confidence = result
        .judged
        .iter()
        .any(|item| item.vulnerable && item.confidence >= 0.5);
    if vulnerable && high_confidence {
        emitter.info("Reflection: vulnerability confirmed — stopping agentic retries");
        return false;
    }
    emitter.info("Reflection: no confirmed vulnerability — preparing retry");
    true
}

pub async fn generate_scan_payloads(
    data_dir: &std::path::Path,
    inference_manager: Arc<AsyncMutex<InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    plan: &AttackPlan,
    config: &ScanExecutionConfig,
    emitter: &ScanProgressEmitter,
) -> Result<HashMap<AttackCategory, Vec<AttackPayload>>, String> {
    emitter.info("Generating attack payloads from Yazg...");
    let pack = if let Some(ref strategy) = config.payload_strategy {
        generate_payloads_for_scan_job_with_strategy(
            data_dir,
            inference_manager,
            model_manager,
            model_provider,
            runtime_manager,
            plan,
            strategy,
        )
        .await
        .map_err(|err| err.to_string())?
    } else if let Some(mode) = parse_generator_mode_optional(config.generator_mode.as_deref()) {
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
        .map_err(|err| err.to_string())?
    } else {
        return Err("payload strategy or generator mode is required before attack".into());
    };

    info!(
        payloads = pack.stats.payload_count,
        categories = pack.stats.category_count,
        "attack payloads ready"
    );
    emitter.info(format!(
        "Generated {} payloads across {} categories",
        pack.stats.payload_count, pack.stats.category_count
    ));
    Ok(prompt_payloads_map(&pack))
}

async fn regenerate_category_payloads(
    data_dir: &std::path::Path,
    inference_manager: Arc<AsyncMutex<InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    plan: &AttackPlan,
    category: AttackCategory,
    strategy: &PayloadStrategy,
    retry: u32,
) -> Result<HashMap<AttackCategory, Vec<AttackPayload>>, String> {
    let mode = escalated_generator_mode(strategy, retry);
    let category_plan = AttackPlan {
        categories: vec![category],
        ..plan.clone()
    };
    let pack = generate_payloads_for_scan_job_with_options(
        data_dir,
        inference_manager,
        model_manager,
        model_provider,
        runtime_manager,
        &category_plan,
        mode,
        Some(strategy.max_total_payloads),
    )
    .await
    .map_err(|err| err.to_string())?;
    Ok(category_payload_map(&prompt_payloads_map(&pack), category))
}

pub struct TargetProfileScanContext<'a> {
    pub repos: &'a Repositories,
    pub scan_id: &'a str,
    pub project_id: &'a str,
    pub target_id: &'a str,
    pub profile: &'a aisec_target_profile::TargetProfile,
    pub categories: &'a [AttackCategory],
    pub disabled_tests: &'a [String],
    pub profile_id: &'a str,
    pub attack_runtime: AttackRuntime,
    pub data_dir: &'a std::path::Path,
    pub inference_manager: Arc<AsyncMutex<InferenceRuntimeManager>>,
    pub model_manager: Arc<AsyncMutex<LocalModelManager>>,
    pub model_provider: SharedModelProvider,
    pub runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    pub plugin_manager: Arc<AsyncMutex<aisec_plugin_host::PluginManager>>,
    pub cancel: Arc<AtomicBool>,
    pub paused: Arc<AtomicBool>,
    pub progress: Arc<Mutex<ScanProgress>>,
    pub emitter: ScanProgressEmitter,
}

pub struct TargetProfileScanOutcome {
    pub findings_total: u64,
    pub had_error: bool,
}

async fn wait_if_paused(paused: &AtomicBool, cancel: &AtomicBool) {
    while paused.load(Ordering::Relaxed) {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
}

async fn wait_pipeline_warmup(
    warmup_secs: u32,
    cancel: &AtomicBool,
    paused: &AtomicBool,
    progress: &Arc<Mutex<ScanProgress>>,
    emitter: &ScanProgressEmitter,
) {
    if warmup_secs == 0 {
        return;
    }

    set_scan_phase(progress, "preparing", Some("loading attack monitor"), None, None);
    emitter.info(format!(
        "Attack pipeline starts in {warmup_secs}s — loading monitor…"
    ));

    let deadline = Instant::now() + Duration::from_secs(u64::from(warmup_secs));
    while Instant::now() < deadline {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        wait_if_paused(paused, cancel).await;
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

pub async fn run_target_profile_attack_scan(
    ctx: TargetProfileScanContext<'_>,
    config: ScanExecutionConfig,
) -> TargetProfileScanOutcome {
    wait_pipeline_warmup(
        config.pipeline_warmup_secs,
        &ctx.cancel,
        &ctx.paused,
        &ctx.progress,
        &ctx.emitter,
    )
    .await;

    if ctx.cancel.load(Ordering::Relaxed) {
        return TargetProfileScanOutcome {
            findings_total: 0,
            had_error: false,
        };
    }

    let plan = attack_plan_from_scan(
        ctx.profile_id.to_string(),
        ctx.categories.to_vec(),
        ctx.disabled_tests.to_vec(),
    );

    set_scan_phase(&ctx.progress, "generate", Some("all categories"), None, None);

    let generated_payloads = match generate_scan_payloads(
        ctx.data_dir,
        Arc::clone(&ctx.inference_manager),
        Arc::clone(&ctx.model_manager),
        ctx.model_provider.clone(),
        Arc::clone(&ctx.runtime_manager),
        &plan,
        &config,
        &ctx.emitter,
    )
    .await
    {
        Ok(map) => map,
        Err(err) => {
            warn!(scan_id = %ctx.scan_id, error = %err, "payload generation failed");
            ctx.emitter.error(format!("Payload generation failed: {err}"));
            return TargetProfileScanOutcome {
                findings_total: 0,
                had_error: true,
            };
        }
    };

    if let Some(ref strategy) = config.payload_strategy {
        if let Err(err) = validate_payload_map_budget(
            &generated_payloads,
            ctx.categories,
            ctx.disabled_tests,
            strategy,
        ) {
            warn!(scan_id = %ctx.scan_id, error = %err, "payload budget not met");
            ctx.emitter.error(err.clone());
            return TargetProfileScanOutcome {
                findings_total: 0,
                had_error: true,
            };
        }
    }

    let mut findings_total = 0u64;
    let mut had_error = false;

    for category in ctx.categories {
        if ctx.cancel.load(Ordering::Relaxed) {
            break;
        }
        wait_if_paused(&ctx.paused, &ctx.cancel).await;
        if ctx.cancel.load(Ordering::Relaxed) {
            break;
        }

        if let Ok(mut state) = ctx.progress.lock() {
            state.status = if ctx.paused.load(Ordering::Relaxed) {
                "paused".into()
            } else {
                "running".into()
            };
            state.current_endpoint = Some(ctx.profile.full_url());
        }

        let run_options = config
            .payload_strategy
            .as_ref()
            .map(|strategy| CategoryRunOptions::from_strategy(*category, ctx.disabled_tests, strategy));

        let category_label = category.display_name();

        let result = if config.execution == ExecutionStrategy::Agentic {
            run_agentic_category(
                &ctx,
                &config,
                &plan,
                *category,
                &generated_payloads,
                run_options.as_ref(),
            )
            .await
        } else {
            run_sequential_category(
                &ctx,
                *category,
                &generated_payloads,
                run_options.as_ref(),
            )
            .await
        };

        match result {
            Ok(category_result) => {
                findings_total += category_result.findings.len() as u64;
                if let Ok(mut state) = ctx.progress.lock() {
                    state.findings = findings_total;
                    state.current_test = Some(category_label.to_string());
                }
            }
            Err(err) => {
                had_error = true;
                ctx.emitter.error(format!("{category_label} failed: {err}"));
            }
        }
    }

    TargetProfileScanOutcome {
        findings_total,
        had_error,
    }
}

async fn run_sequential_category(
    ctx: &TargetProfileScanContext<'_>,
    category: AttackCategory,
    generated_payloads: &HashMap<AttackCategory, Vec<AttackPayload>>,
    run_options: Option<&CategoryRunOptions>,
) -> Result<CategoryRunResult, String> {
    run_category_on_target_profile(
        ctx.repos,
        ctx.scan_id,
        ctx.project_id,
        ctx.target_id,
        ctx.profile,
        category,
        ctx.attack_runtime.clone(),
        ctx.data_dir,
        Arc::clone(&ctx.inference_manager),
        Arc::clone(&ctx.model_manager),
        ctx.model_provider.clone(),
        Arc::clone(&ctx.runtime_manager),
        ctx.plugin_manager.clone(),
        Some(generated_payloads),
        Some(&ctx.emitter),
        run_options,
        Some(&ctx.progress),
    )
    .await
    .map_err(|err| err.to_string())
}

async fn run_agentic_category(
    ctx: &TargetProfileScanContext<'_>,
    config: &ScanExecutionConfig,
    plan: &AttackPlan,
    category: AttackCategory,
    generated_payloads: &HashMap<AttackCategory, Vec<AttackPayload>>,
    run_options: Option<&CategoryRunOptions>,
) -> Result<CategoryRunResult, String> {
    let strategy = config
        .payload_strategy
        .clone()
        .ok_or_else(|| "agentic execution requires payload strategy".to_string())?;

    let mut last_result: Option<CategoryRunResult> = None;
    let category_label = category.display_name();

    for attempt in 1..=config.max_attempts {
        if ctx.cancel.load(Ordering::Relaxed) {
            break;
        }

        let retry = attempt.saturating_sub(1);
        set_scan_phase(
            &ctx.progress,
            "generate",
            Some(&category_label),
            Some(attempt),
            Some(retry),
        );
        ctx.emitter.info(format!(
            "Agentic attempt {attempt}/{} — generating payloads for {category_label}",
            config.max_attempts
        ));

        let payloads_for_run = if attempt == 1 {
            category_payload_map(generated_payloads, category)
        } else {
            regenerate_category_payloads(
                ctx.data_dir,
                Arc::clone(&ctx.inference_manager),
                Arc::clone(&ctx.model_manager),
                ctx.model_provider.clone(),
                Arc::clone(&ctx.runtime_manager),
                plan,
                category,
                &strategy,
                retry,
            )
            .await?
        };

        set_scan_phase(
            &ctx.progress,
            "attack",
            Some(&category_label),
            Some(attempt),
            Some(retry),
        );

        let result = run_category_on_target_profile(
            ctx.repos,
            ctx.scan_id,
            ctx.project_id,
            ctx.target_id,
            ctx.profile,
            category,
            ctx.attack_runtime.clone(),
            ctx.data_dir,
            Arc::clone(&ctx.inference_manager),
            Arc::clone(&ctx.model_manager),
            ctx.model_provider.clone(),
            Arc::clone(&ctx.runtime_manager),
            ctx.plugin_manager.clone(),
            Some(&payloads_for_run),
            Some(&ctx.emitter),
            run_options,
            Some(&ctx.progress),
        )
        .await
        .map_err(|err| err.to_string())?;

        set_scan_phase(
            &ctx.progress,
            "judge",
            Some(&category_label),
            Some(attempt),
            Some(retry),
        );

        if config.reflection_enabled {
            set_scan_phase(
                &ctx.progress,
                "reflection",
                Some(&category_label),
                Some(attempt),
                Some(retry),
            );
        }

        let should_retry = reflection_allows_retry(config.reflection_enabled, &result, &ctx.emitter);
        last_result = Some(result);

        if !should_retry {
            break;
        }

        if attempt >= config.max_attempts {
            break;
        }

        set_scan_phase(
            &ctx.progress,
            "retry",
            Some(&category_label),
            Some(attempt + 1),
            Some(attempt),
        );
        ctx.emitter.info(format!(
            "Retrying {category_label} (attempt {} of {})",
            attempt + 1,
            config.max_attempts
        ));
    }

    last_result.ok_or_else(|| "agentic category produced no attempts".into())
}
