//! Scan attack orchestration — payload preparation and sequential/agentic flows.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use promptlab_agent::{
    AttackAttemptObservation, AttackExecutionLlms, AttackExecutionRequest, AttackExecutionTools,
    AdaptPlanOutcome, EndpointPacing, MemoryContext, SequentialAttackExecutionRequest,
    YazgSupervisor,
};
use promptlab_attack::{AttackCategory, AttackPayload};
use promptlab_inference::InferenceRuntimeManager;
use promptlab_models::LocalModelManager;
use promptlab_planner::AttackPlan;
use promptlab_runtime::{RuntimeManager, SharedModelProvider};
use promptlab_storage::Repositories;
use promptlab_target_profile::{MutationLevel, PayloadGenerationStrategy, PayloadStrategy};
use promptlab_target_profile::wizard_plan::{
    enabled_tests_for_category, estimate_scan_requests, ExecutionStrategy,
};
use async_trait::async_trait;
use tauri::async_runtime::Mutex as AsyncMutex;
use tracing::{info, warn};

use crate::agent_memory::SqliteAgentMemoryStore;
use crate::commands::attack::{
    CategoryRunOptions, CategoryRunResult, JudgedAttemptSummary, run_category_on_target_profile,
};
use crate::commands::generator::{
    attack_plan_from_scan, generate_payloads_for_scan_job_with_options_and_catalog,
    generate_payloads_for_scan_job_with_strategy_context_and_catalog,
    parse_generator_mode_optional, prompt_payloads_map, validate_payload_map_budget,
};
use crate::events::ScanProgressEmitter;
use crate::inference_host::{is_inference_ready, YazgHostLlms};
use crate::jobs::{bump_scan_progress, ScanProgress};
use crate::session_auth::AttackRuntime;

pub struct ScanExecutionConfig {
    pub execution: ExecutionStrategy,
    pub max_attempts: u32,
    pub reflection_enabled: bool,
    pub adaptive_planning: bool,
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
        adaptive_planning: bool,
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
            adaptive_planning: agentic && adaptive_planning,
            payload_strategy,
            generator_mode,
            pipeline_warmup_secs: 3,
        }
    }
}

pub fn scan_testcases_total(
    categories: &[AttackCategory],
    disabled_tests: &[String],
) -> u64 {
    categories
        .iter()
        .map(|category| u64::from(enabled_tests_for_category(*category, disabled_tests)))
        .sum::<u64>()
        .max(1)
}

pub fn scan_attack_requests_total(
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

pub fn scan_progress_total(
    categories: &[AttackCategory],
    disabled_tests: &[String],
    config: &ScanExecutionConfig,
) -> u64 {
    let strategy = config.payload_strategy.clone().unwrap_or_default();
    let attack_units = estimate_scan_requests(
        categories,
        disabled_tests,
        &strategy,
        config.execution,
        config.max_attempts,
    )
    .max(1) as u64;

    let active_categories = categories
        .iter()
        .filter(|category| enabled_tests_for_category(**category, disabled_tests) > 0)
        .count()
        .max(1) as u64;
    let attempts = u64::from(config.max_attempts.max(1));

    let pipeline_units = match config.execution {
        ExecutionStrategy::Sequential => {
            // preparing + generate + attack payloads + judge payloads
            2 + attack_units.saturating_mul(2)
        }
        ExecutionStrategy::Agentic => {
            let generate_units = active_categories.saturating_mul(attempts);
            let reflection_units = if config.reflection_enabled {
                active_categories.saturating_mul(attempts)
            } else {
                0
            };
            let adaptive_units = if config.adaptive_planning && attempts > 1 {
                active_categories.saturating_mul(attempts - 1)
            } else {
                0
            };
            let retry_units = if attempts > 1 {
                active_categories.saturating_mul(attempts - 1)
            } else {
                0
            };
            1 + generate_units
                + attack_units.saturating_mul(2)
                + reflection_units
                + adaptive_units
                + retry_units
        }
    };

    pipeline_units.max(1)
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

fn observation_from_category_result(result: &CategoryRunResult) -> AttackAttemptObservation {
    let high_confidence_vuln = result
        .judged
        .iter()
        .any(|item| item.vulnerable && item.confidence >= 0.5);
    let summary = result
        .judged
        .iter()
        .take(8)
        .map(|j| {
            format!(
                "{} vul={} conf={:.2} ({})",
                j.payload_id, j.vulnerable, j.confidence, j.summary
            )
        })
        .collect::<Vec<_>>()
        .join(" | ");
    AttackAttemptObservation {
        successes: result.successes,
        attempts: result.attempts as u64,
        any_vulnerable: category_any_vulnerable(result),
        high_confidence_vuln,
        summary,
        http_successes: result.http_successes,
        transport_errors: result.transport_errors,
        rate_limited: result.rate_limited,
        server_errors: result.server_errors,
        avg_latency_ms: result.avg_latency_ms,
        max_latency_ms: result.max_latency_ms,
        endpoint_unhealthy: result.endpoint_unhealthy,
        endpoint_error: None,
    }
}

fn category_result_produced_no_requests(result: &CategoryRunResult) -> bool {
    result.attempts == 0
        && result.http_successes == 0
        && result.transport_errors == 0
        && result.rate_limited == 0
        && result.server_errors == 0
}

fn merge_run_options_with_pacing(
    base: Option<&CategoryRunOptions>,
    pacing: &EndpointPacing,
    category: AttackCategory,
    disabled_tests: &[String],
    strategy: Option<&PayloadStrategy>,
) -> CategoryRunOptions {
    let mut opts = if let Some(base) = base {
        base.clone()
    } else if let Some(strategy) = strategy {
        CategoryRunOptions::from_strategy(category, disabled_tests, strategy)
    } else {
        CategoryRunOptions {
            max_payloads: 20,
            variants_per_test: 1,
            max_concurrent_requests: None,
            inter_request_delay_ms: None,
            timeout_ms: None,
            enabled_mutators: None,
        }
    };
    opts = opts.with_pacing(pacing);
    opts
}

fn technique_id_from_payload_id(payload_id: &str) -> &str {
    payload_id.split(':').next().unwrap_or(payload_id)
}

fn escalate_payload_strategy(strategy: &PayloadStrategy) -> PayloadStrategy {
    PayloadStrategy {
        strategy: strategy.strategy.escalate(),
        mutation_level: strategy.mutation_level.escalate(),
        variants_per_test: (strategy.variants_per_test.saturating_add(2)).min(20),
        max_total_payloads: strategy.max_total_payloads,
        enable_context_awareness: true,
        enable_conversation_memory: strategy.enable_conversation_memory,
        enable_response_adaptation: true,
        enable_payload_deduplication: strategy.enable_payload_deduplication,
        enable_cross_category_mutation: strategy.enable_cross_category_mutation
            || matches!(strategy.mutation_level, MutationLevel::High | MutationLevel::Extreme),
        enabled_mutators: strategy.enabled_mutators.clone(),
    }
    .clamp()
}

/// Replan technique selection + payload strategy for the next agentic attempt.
fn adapt_plan_for_retry(
    plan: &AttackPlan,
    category: AttackCategory,
    strategy: &PayloadStrategy,
    last_result: &CategoryRunResult,
    catalog: &promptlab_payload::PayloadDatabase,
) -> (AttackPlan, PayloadStrategy, Vec<String>) {
    let mut notes = Vec::new();
    let next_strategy = escalate_payload_strategy(strategy);
    if next_strategy.mutation_level != strategy.mutation_level {
        notes.push(format!(
            "escalated mutationLevel {:?} → {:?}",
            strategy.mutation_level, next_strategy.mutation_level
        ));
    }
    if next_strategy.strategy != strategy.strategy {
        notes.push(format!(
            "escalated generation strategy {:?} → {:?}",
            strategy.strategy, next_strategy.strategy
        ));
    }
    if next_strategy.variants_per_test != strategy.variants_per_test {
        notes.push(format!(
            "raised variantsPerTest {} → {}",
            strategy.variants_per_test, next_strategy.variants_per_test
        ));
    }
    if !strategy.enable_response_adaptation {
        notes.push("enabled responseAdaptation for judge-guided retries".into());
    }

    let payload_cat = promptlab_generator::convert::attack_to_payload_category(category);
    let catalog_ids: Vec<String> = catalog
        .by_category(payload_cat)
        .into_iter()
        .map(|record| record.id.clone())
        .collect();

    let tried: std::collections::HashSet<String> = last_result
        .judged
        .iter()
        .map(|item| technique_id_from_payload_id(&item.payload_id).to_string())
        .collect();
    let failed: std::collections::HashSet<String> = last_result
        .judged
        .iter()
        .filter(|item| !item.vulnerable)
        .map(|item| technique_id_from_payload_id(&item.payload_id).to_string())
        .collect();

    let mut disabled: std::collections::HashSet<String> =
        plan.disabled_tests.iter().cloned().collect();
    // Keep disables outside this category untouched; only rotate within category.
    let untried: Vec<String> = catalog_ids
        .iter()
        .filter(|id| !tried.contains(*id) && !disabled.contains(*id))
        .cloned()
        .collect();

    if !untried.is_empty() {
        for id in &failed {
            if catalog_ids.iter().any(|cid| cid == id) {
                disabled.insert(id.clone());
            }
        }
        for id in &untried {
            disabled.remove(id);
        }
        // Ensure at least one technique remains enabled in this category.
        let enabled_count = catalog_ids
            .iter()
            .filter(|id| !disabled.contains(*id))
            .count();
        if enabled_count == 0 {
            if let Some(first) = catalog_ids.first() {
                disabled.remove(first);
            }
        }
        notes.push(format!(
            "rotated techniques: prefer {} untried id(s), de-emphasize {} failed id(s)",
            untried.len(),
            failed.len()
        ));
    } else if !failed.is_empty() {
        notes.push(
            "all category techniques already tried — keeping selection and escalating payload strategy"
                .into(),
        );
    }

    let mut next_plan = plan.clone();
    let mut disabled_list: Vec<String> = disabled.into_iter().collect();
    disabled_list.sort();
    next_plan.disabled_tests = disabled_list;
    next_plan.summary = format!(
        "{} | adaptive replan for {}",
        plan.summary,
        category.as_str()
    );

    (next_plan, next_strategy, notes)
}

pub async fn generate_scan_payloads(
    data_dir: &std::path::Path,
    inference_manager: Arc<AsyncMutex<InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    plan: &AttackPlan,
    config: &ScanExecutionConfig,
    profile: &promptlab_target_profile::TargetProfile,
    catalog: promptlab_payload::PayloadDatabase,
    emitter: &ScanProgressEmitter,
) -> Result<HashMap<AttackCategory, Vec<AttackPayload>>, String> {
    emitter.info("Generating attack payloads from Yazg...");
    let pack = if let Some(ref strategy) = config.payload_strategy {
        generate_payloads_for_scan_job_with_strategy_context_and_catalog(
            data_dir,
            inference_manager,
            model_manager,
            model_provider,
            runtime_manager,
            plan,
            strategy,
            Some(profile),
            None,
            Some(catalog),
        )
        .await
        .map_err(|err| err.to_string())?
    } else if let Some(mode) = parse_generator_mode_optional(config.generator_mode.as_deref()) {
        generate_payloads_for_scan_job_with_options_and_catalog(
            data_dir,
            inference_manager,
            model_manager,
            model_provider,
            runtime_manager,
            plan,
            mode,
            None,
            catalog,
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
    profile: &promptlab_target_profile::TargetProfile,
    catalog: promptlab_payload::PayloadDatabase,
    adaptation_feedback: Option<String>,
    _retry: u32,
) -> Result<HashMap<AttackCategory, Vec<AttackPayload>>, String> {
    let category_plan = AttackPlan {
        categories: vec![category],
        ..plan.clone()
    };
    let pack = generate_payloads_for_scan_job_with_strategy_context_and_catalog(
        data_dir,
        inference_manager,
        model_manager,
        model_provider,
        runtime_manager,
        &category_plan,
        strategy,
        Some(profile),
        adaptation_feedback,
        Some(catalog),
    )
    .await
    .map_err(|err| err.to_string())?;
    Ok(category_payload_map(&prompt_payloads_map(&pack), category))
}

/// Auto re-runs for a category after it fails in the host category loop.
pub const MAX_CATEGORY_AUTO_RETRIES: u32 = 3;

/// Per-scan pacing memory so auto-retry / next category keep escalated endpoint limits.
#[derive(Debug, Default, Clone)]
pub struct ScanPacingCache {
    by_category: HashMap<String, EndpointPacing>,
    last: Option<EndpointPacing>,
}

impl ScanPacingCache {
    fn resolve_initial(&self, category: &str) -> EndpointPacing {
        self.by_category
            .get(category)
            .cloned()
            .or_else(|| self.last.clone())
            .unwrap_or_default()
    }

    fn remember(&mut self, category: &str, pacing: EndpointPacing) {
        self.by_category
            .insert(category.to_string(), pacing.clone());
        self.last = Some(pacing);
    }
}

pub struct TargetProfileScanContext<'a> {
    pub repos: &'a Repositories,
    pub scan_id: &'a str,
    pub project_id: &'a str,
    pub target_id: &'a str,
    pub profile: &'a promptlab_target_profile::TargetProfile,
    pub categories: &'a [AttackCategory],
    pub disabled_tests: &'a [String],
    pub profile_id: &'a str,
    pub attack_runtime: AttackRuntime,
    pub data_dir: &'a std::path::Path,
    pub inference_manager: Arc<AsyncMutex<InferenceRuntimeManager>>,
    pub model_manager: Arc<AsyncMutex<LocalModelManager>>,
    pub model_provider: SharedModelProvider,
    pub runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    pub plugin_manager: Arc<AsyncMutex<promptlab_plugin_host::PluginManager>>,
    pub cancel: Arc<AtomicBool>,
    pub paused: Arc<AtomicBool>,
    pub job_controls: Option<crate::jobs::ScanJobControls>,
    pub progress: Arc<Mutex<ScanProgress>>,
    pub emitter: ScanProgressEmitter,
    /// When true (default), skip categories already counted in `categories_completed`.
    /// Set false for manual "retry failed only" jobs that pass only failed categories.
    pub skip_completed_categories: bool,
    /// Survives across categories and host auto-retries within one scan job.
    pub pacing_cache: Arc<Mutex<ScanPacingCache>>,
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
        bump_scan_progress(progress, 1);
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

    bump_scan_progress(progress, 1);
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

    let catalog = match crate::attack_catalog::load_payload_database_from_repos(ctx.repos).await {
        Ok(db) => db,
        Err(err) => {
            warn!(scan_id = %ctx.scan_id, error = %err, "attack catalog load failed");
            ctx.emitter
                .error(format!("Attack catalog load failed: {err}"));
            return TargetProfileScanOutcome {
                findings_total: 0,
                had_error: true,
            };
        }
    };

    let generated_payloads = match generate_scan_payloads(
        ctx.data_dir,
        Arc::clone(&ctx.inference_manager),
        Arc::clone(&ctx.model_manager),
        ctx.model_provider.clone(),
        Arc::clone(&ctx.runtime_manager),
        &plan,
        &config,
        ctx.profile,
        catalog.clone(),
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

    if config.execution == ExecutionStrategy::Sequential {
        bump_scan_progress(&ctx.progress, 1);
    }

    let mut findings_total = ctx
        .progress
        .lock()
        .map(|state| state.findings)
        .unwrap_or(0);
    let mut had_error = false;

    let categories_completed = ctx
        .progress
        .lock()
        .map(|state| state.categories_completed as usize)
        .unwrap_or(0);

    for (category_index, category) in ctx.categories.iter().enumerate() {
        if ctx.skip_completed_categories && category_index < categories_completed {
            continue;
        }
        if ctx.cancel.load(Ordering::Relaxed) {
            break;
        }
        wait_if_paused(&ctx.paused, &ctx.cancel).await;
        if ctx.cancel.load(Ordering::Relaxed) {
            break;
        }

        let outcome = run_category_pass(
            &ctx,
            &config,
            &plan,
            *category,
            &generated_payloads,
            &catalog,
            CategoryPassKind::Initial,
        )
        .await;

        if ctx.paused.load(Ordering::Relaxed) {
            break;
        }

        match outcome {
            Ok(category_result) => {
                findings_total += category_result.findings.len() as u64;
                record_category_success(
                    &ctx.progress,
                    findings_total,
                    category,
                    ctx.skip_completed_categories,
                );
            }
            Err(err) => {
                had_error = true;
                record_category_failure(&ctx, *category, &err, ctx.skip_completed_categories);
            }
        }
    }

    // Auto-retry failed categories up to MAX_CATEGORY_AUTO_RETRIES times each.
    let mut auto_retries: HashMap<String, u32> = HashMap::new();
    loop {
        if ctx.cancel.load(Ordering::Relaxed) || ctx.paused.load(Ordering::Relaxed) {
            break;
        }

        let failed_ids = ctx
            .progress
            .lock()
            .map(|state| state.categories_failed.clone())
            .unwrap_or_default();
        if failed_ids.is_empty() {
            break;
        }

        let mut retried_any = false;
        for failed_id in failed_ids {
            let attempts = auto_retries.get(&failed_id).copied().unwrap_or(0);
            if attempts >= MAX_CATEGORY_AUTO_RETRIES {
                continue;
            }
            let Some(category) = ctx
                .categories
                .iter()
                .copied()
                .find(|c| c.as_str() == failed_id)
                .or_else(|| parse_category_id(&failed_id))
            else {
                continue;
            };

            retried_any = true;
            let attempt = attempts.saturating_add(1);
            auto_retries.insert(failed_id.clone(), attempt);

            ctx.emitter.info(format!(
                "Yazg auto-retry {} ({attempt}/{MAX_CATEGORY_AUTO_RETRIES}) after failure",
                category.display_name()
            ));

            wait_if_paused(&ctx.paused, &ctx.cancel).await;
            if ctx.cancel.load(Ordering::Relaxed) || ctx.paused.load(Ordering::Relaxed) {
                break;
            }

            let outcome = run_category_pass(
                &ctx,
                &config,
                &plan,
                category,
                &generated_payloads,
                &catalog,
                CategoryPassKind::AutoRetry { attempt },
            )
            .await;

            if ctx.paused.load(Ordering::Relaxed) {
                break;
            }

            match outcome {
                Ok(category_result) => {
                    findings_total += category_result.findings.len() as u64;
                    record_category_success(&ctx.progress, findings_total, &category, false);
                    ctx.emitter.info(format!(
                        "{} recovered on auto-retry {attempt}/{MAX_CATEGORY_AUTO_RETRIES}",
                        category.display_name()
                    ));
                }
                Err(err) => {
                    had_error = true;
                    record_category_failure(&ctx, category, &err, false);
                    if attempt >= MAX_CATEGORY_AUTO_RETRIES {
                        ctx.emitter.error(format!(
                            "{} still failing after {MAX_CATEGORY_AUTO_RETRIES} auto-retries",
                            category.display_name()
                        ));
                    }
                }
            }
        }

        if !retried_any {
            break;
        }
    }

    had_error = ctx
        .progress
        .lock()
        .map(|state| !state.categories_failed.is_empty())
        .unwrap_or(had_error);

    TargetProfileScanOutcome {
        findings_total,
        had_error,
    }
}

#[derive(Debug, Clone, Copy)]
enum CategoryPassKind {
    Initial,
    AutoRetry { attempt: u32 },
}

fn parse_category_id(id: &str) -> Option<AttackCategory> {
    AttackCategory::all()
        .iter()
        .copied()
        .find(|c| c.as_str() == id || c.display_name().eq_ignore_ascii_case(id))
}

fn record_category_success(
    progress: &Arc<Mutex<ScanProgress>>,
    findings_total: u64,
    category: &AttackCategory,
    bump_completed: bool,
) {
    let id = category.as_str();
    if let Ok(mut state) = progress.lock() {
        state.findings = findings_total;
        if bump_completed {
            state.categories_completed = state.categories_completed.saturating_add(1);
        }
        state.categories_failed.retain(|c| c != id);
    }
}

fn record_category_failure(
    ctx: &TargetProfileScanContext<'_>,
    category: AttackCategory,
    err: &str,
    bump_completed: bool,
) {
    let category_label = category.display_name();
    ctx.emitter
        .error(format!("{category_label} failed: {err}"));
    if let Ok(mut state) = ctx.progress.lock() {
        if bump_completed {
            state.categories_completed = state.categories_completed.saturating_add(1);
        }
        let id = category.as_str().to_string();
        if !state.categories_failed.iter().any(|c| c == &id) {
            state.categories_failed.push(id);
        }
    }
}

async fn run_category_pass(
    ctx: &TargetProfileScanContext<'_>,
    config: &ScanExecutionConfig,
    plan: &AttackPlan,
    category: AttackCategory,
    generated_payloads: &HashMap<AttackCategory, Vec<AttackPayload>>,
    catalog: &promptlab_payload::PayloadDatabase,
    kind: CategoryPassKind,
) -> Result<CategoryRunResult, String> {
    if let Ok(mut state) = ctx.progress.lock() {
        state.status = if ctx.paused.load(Ordering::Relaxed) {
            "paused".into()
        } else {
            "running".into()
        };
        state.current_endpoint = Some(ctx.profile.full_url());
        // Clear failed marker while this category is actively being (re)attempted.
        let id = category.as_str();
        state.categories_failed.retain(|c| c != id);
        if let CategoryPassKind::AutoRetry { attempt } = kind {
            state.current_retry = Some(attempt);
        } else {
            state.current_retry = None;
        }
    }

    let run_options = config
        .payload_strategy
        .as_ref()
        .map(|strategy| CategoryRunOptions::from_strategy(category, ctx.disabled_tests, strategy));

    if config.execution == ExecutionStrategy::Agentic {
        run_agentic_category(
            ctx,
            config,
            plan,
            category,
            generated_payloads,
            catalog,
            run_options.as_ref(),
        )
        .await
    } else {
        run_sequential_category(ctx, category, generated_payloads, run_options.as_ref()).await
    }
}

async fn run_sequential_category(
    ctx: &TargetProfileScanContext<'_>,
    category: AttackCategory,
    generated_payloads: &HashMap<AttackCategory, Vec<AttackPayload>>,
    run_options: Option<&CategoryRunOptions>,
) -> Result<CategoryRunResult, String> {
    let initial_pacing = ctx
        .pacing_cache
        .lock()
        .map(|cache| cache.resolve_initial(category.as_str()))
        .unwrap_or_default();
    if !initial_pacing.is_default() {
        ctx.emitter.info(format!(
            "SequentialAttackExecutionAgent: inheriting pacing for {} — {}",
            category.display_name(),
            initial_pacing.summary()
        ));
    }

    let tools = SequentialCategoryTools {
        ctx,
        category,
        initial_payloads: generated_payloads,
        initial_run_options: run_options,
        state: Mutex::new(SequentialCategoryState {
            payloads: category_payload_map(generated_payloads, category),
            last_result: None,
            pacing: initial_pacing,
        }),
    };

    let memory = SqliteAgentMemoryStore::new(ctx.repos.clone());
    let memory_ctx = MemoryContext::new(format!(
        "scan-seq:{}:{}",
        ctx.scan_id,
        category.as_str()
    ))
    .with_project(Some(ctx.project_id.to_string()))
    .with_target(Some(ctx.target_id.to_string()))
    .with_scan(Some(ctx.scan_id.to_string()));

    let request = SequentialAttackExecutionRequest {
        category: category.as_str().to_string(),
        max_react_steps: 24,
    };

    let llm_ready = {
        let inference = ctx.inference_manager.lock().await;
        is_inference_ready(&inference)
    };
    let hosts = YazgHostLlms::from_app(
        ctx.data_dir.to_path_buf(),
        Arc::clone(&ctx.inference_manager),
        Arc::clone(&ctx.model_manager),
        ctx.model_provider.clone(),
        Arc::clone(&ctx.runtime_manager),
    );
    let react = hosts.react_llms();

    ctx.emitter.info(format!(
        "Yazg → SequentialAttackExecutionAgent for {} (endpoint recovery ReAct)",
        category.display_name()
    ));

    let agent_result = YazgSupervisor::execute_sequential_attack(
        &request,
        &tools,
        Some(react.supervisor),
        llm_ready,
        Some(&memory),
        memory_ctx,
    )
    .await;

    if let Ok(mut cache) = ctx.pacing_cache.lock() {
        if let Ok(state) = tools.state.lock() {
            cache.remember(category.as_str(), state.pacing.clone());
        }
    }

    match agent_result {
        Ok(_) => {
            // ReAct / Info events were already live-emitted via emit_and_record.
            tools
                .state
                .lock()
                .ok()
                .and_then(|s| s.last_result.clone())
                .ok_or_else(|| "sequential category produced no attempts".into())
        }
        Err(err) => Err(err.to_string()),
    }
}

struct SequentialCategoryState {
    payloads: HashMap<AttackCategory, Vec<AttackPayload>>,
    last_result: Option<CategoryRunResult>,
    pacing: EndpointPacing,
}

struct SequentialCategoryTools<'a> {
    ctx: &'a TargetProfileScanContext<'a>,
    category: AttackCategory,
    initial_payloads: &'a HashMap<AttackCategory, Vec<AttackPayload>>,
    initial_run_options: Option<&'a CategoryRunOptions>,
    state: Mutex<SequentialCategoryState>,
}

#[async_trait]
impl AttackExecutionTools for SequentialCategoryTools<'_> {
    fn is_cancelled(&self) -> bool {
        self.ctx.cancel.load(Ordering::Relaxed)
    }

    async fn wait_if_paused(&self) {
        wait_if_paused(&self.ctx.paused, &self.ctx.cancel).await;
    }

    async fn set_phase(&self, phase: &str, attempt: u32, retry: u32) {
        let label = self.category.display_name();
        set_scan_phase(
            &self.ctx.progress,
            phase,
            Some(label),
            Some(attempt),
            Some(retry),
        );
    }

    async fn bump_progress(&self, delta: u64) {
        bump_scan_progress(&self.ctx.progress, delta);
    }

    async fn emit_info(&self, message: String) {
        self.ctx.emitter.info(message);
    }

    async fn generate_payloads(
        &self,
        _attempt: u32,
        _focus_hints: &[String],
    ) -> Result<(), String> {
        // Sequential uses pre-generated payloads from the scan prepare phase.
        let map = category_payload_map(self.initial_payloads, self.category);
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        state.payloads = map;
        Ok(())
    }

    async fn run_attack_attempt(
        &self,
        _attempt: u32,
    ) -> Result<AttackAttemptObservation, String> {
        let (payloads, pacing) = {
            let state = self.state.lock().map_err(|e| e.to_string())?;
            (state.payloads.clone(), state.pacing.clone())
        };

        let payload_count = payloads.get(&self.category).map(|v| v.len()).unwrap_or(0);
        if payload_count == 0 {
            return Err(format!(
                "no payloads available for {}",
                self.category.as_str()
            ));
        }

        let owned_options = merge_run_options_with_pacing(
            self.initial_run_options,
            &pacing,
            self.category,
            &[],
            None,
        );

        let result = run_category_on_target_profile(
            self.ctx.repos,
            self.ctx.scan_id,
            self.ctx.project_id,
            self.ctx.target_id,
            self.ctx.profile,
            self.category,
            self.ctx.attack_runtime.clone(),
            self.ctx.data_dir,
            Arc::clone(&self.ctx.inference_manager),
            Arc::clone(&self.ctx.model_manager),
            self.ctx.model_provider.clone(),
            Arc::clone(&self.ctx.runtime_manager),
            self.ctx.plugin_manager.clone(),
            Some(&payloads),
            Some(&self.ctx.emitter),
            Some(&owned_options),
            Some(&self.ctx.progress),
            self.ctx.job_controls.as_ref(),
        )
        .await
        .map_err(|err| err.to_string())?;

        if category_result_produced_no_requests(&result) {
            return Err(format!(
                "attack produced no requests for {}",
                self.category.as_str()
            ));
        }

        let obs = observation_from_category_result(&result);
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        state.last_result = Some(result);
        Ok(obs)
    }

    async fn apply_adapt(&self, _adapt: &AdaptPlanOutcome) -> Result<(), String> {
        Ok(())
    }

    async fn current_pacing(&self) -> EndpointPacing {
        self.state
            .lock()
            .map(|s| s.pacing.clone())
            .unwrap_or_default()
    }

    async fn apply_pacing(&self, pacing: &EndpointPacing) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        state.pacing = pacing.clone();
        self.ctx.emitter.info(format!(
            "Sequential pacing updated: {}",
            pacing.summary()
        ));
        Ok(())
    }

    async fn wait_backoff(&self, delay_ms: u64) {
        if delay_ms == 0 {
            return;
        }
        self.ctx.emitter.info(format!(
            "Sequential endpoint backoff waiting {delay_ms}ms before retry"
        ));
        tokio::time::sleep(Duration::from_millis(delay_ms.min(60_000))).await;
    }
}

async fn run_agentic_category(
    ctx: &TargetProfileScanContext<'_>,
    config: &ScanExecutionConfig,
    plan: &AttackPlan,
    category: AttackCategory,
    generated_payloads: &HashMap<AttackCategory, Vec<AttackPayload>>,
    catalog: &promptlab_payload::PayloadDatabase,
    run_options: Option<&CategoryRunOptions>,
) -> Result<CategoryRunResult, String> {
    let strategy = config
        .payload_strategy
        .clone()
        .ok_or_else(|| "agentic execution requires payload strategy".to_string())?;

    let initial_pacing = ctx
        .pacing_cache
        .lock()
        .map(|cache| cache.resolve_initial(category.as_str()))
        .unwrap_or_default();
    if !initial_pacing.is_default() {
        ctx.emitter.info(format!(
            "AgenticAttackExecutionAgent: inheriting pacing for {} — {}",
            category.display_name(),
            initial_pacing.summary()
        ));
    }

    let tools = AgenticCategoryTools {
        ctx,
        config,
        catalog,
        category,
        initial_payloads: generated_payloads,
        initial_run_options: run_options,
        state: Mutex::new(AgenticCategoryState {
            plan: plan.clone(),
            strategy: strategy.clone(),
            payloads: category_payload_map(generated_payloads, category),
            last_result: None,
            focus_hints: Vec::new(),
            pacing: initial_pacing,
        }),
    };

    let llm_ready = {
        let inference = ctx.inference_manager.lock().await;
        is_inference_ready(&inference)
    };

    let hosts = YazgHostLlms::from_app(
        ctx.data_dir.to_path_buf(),
        Arc::clone(&ctx.inference_manager),
        Arc::clone(&ctx.model_manager),
        ctx.model_provider.clone(),
        Arc::clone(&ctx.runtime_manager),
    );
    let react = hosts.react_llms();
    let exec_llms = AttackExecutionLlms {
        orchestrator: react.supervisor,
        reflection: react.supervisor,
        plan: react.plan,
        llm_ready,
    };

    let request = AttackExecutionRequest {
        category: category.as_str().to_string(),
        max_attempts: config.max_attempts.max(1),
        reflection_enabled: config.reflection_enabled,
        adaptive_planning: config.adaptive_planning,
        mutation_level: format!("{:?}", strategy.mutation_level),
        generation_strategy: format!("{:?}", strategy.strategy),
        variants_per_test: strategy.variants_per_test.min(20) as u8,
        response_adaptation: strategy.enable_response_adaptation,
        max_react_steps: (config.max_attempts.max(1) as usize)
            .saturating_mul(8)
            .max(16),
    };

    ctx.emitter.info(format!(
        "Yazg → AgenticAttackExecutionAgent for {} (agentic)",
        category.display_name()
    ));

    let memory = SqliteAgentMemoryStore::new(ctx.repos.clone());
    let memory_ctx = MemoryContext::new(format!(
        "scan-exec:{}:{}",
        ctx.scan_id,
        category.as_str()
    ))
    .with_project(Some(ctx.project_id.to_string()))
    .with_target(Some(ctx.target_id.to_string()))
    .with_scan(Some(ctx.scan_id.to_string()));

    let agent_result =
        YazgSupervisor::execute_attack(&request, &tools, &exec_llms, Some(&memory), memory_ctx)
            .await;

    if let Ok(mut cache) = ctx.pacing_cache.lock() {
        if let Ok(state) = tools.state.lock() {
            cache.remember(category.as_str(), state.pacing.clone());
        }
    }

    match agent_result {
        Ok(_) => {
            // ReAct / Info events were already live-emitted via emit_and_record.
            tools
                .state
                .lock()
                .ok()
                .and_then(|s| s.last_result.clone())
                .ok_or_else(|| "agentic category produced no attempts".into())
        }
        Err(err) => Err(err.to_string()),
    }
}

struct AgenticCategoryState {
    plan: AttackPlan,
    strategy: PayloadStrategy,
    payloads: HashMap<AttackCategory, Vec<AttackPayload>>,
    last_result: Option<CategoryRunResult>,
    focus_hints: Vec<String>,
    pacing: EndpointPacing,
}

struct AgenticCategoryTools<'a> {
    ctx: &'a TargetProfileScanContext<'a>,
    config: &'a ScanExecutionConfig,
    catalog: &'a promptlab_payload::PayloadDatabase,
    category: AttackCategory,
    initial_payloads: &'a HashMap<AttackCategory, Vec<AttackPayload>>,
    initial_run_options: Option<&'a CategoryRunOptions>,
    state: Mutex<AgenticCategoryState>,
}

#[async_trait]
impl AttackExecutionTools for AgenticCategoryTools<'_> {
    fn is_cancelled(&self) -> bool {
        self.ctx.cancel.load(Ordering::Relaxed)
    }

    async fn wait_if_paused(&self) {
        wait_if_paused(&self.ctx.paused, &self.ctx.cancel).await;
    }

    async fn set_phase(&self, phase: &str, attempt: u32, retry: u32) {
        let label = self.category.display_name();
        set_scan_phase(
            &self.ctx.progress,
            phase,
            Some(label),
            Some(attempt),
            Some(retry),
        );
    }

    async fn bump_progress(&self, delta: u64) {
        bump_scan_progress(&self.ctx.progress, delta);
    }

    async fn emit_info(&self, message: String) {
        self.ctx.emitter.info(message);
    }

    async fn generate_payloads(
        &self,
        attempt: u32,
        focus_hints: &[String],
    ) -> Result<(), String> {
        {
            let mut state = self.state.lock().map_err(|e| e.to_string())?;
            state.focus_hints = focus_hints.to_vec();
        }

        if attempt <= 1 {
            let map = category_payload_map(self.initial_payloads, self.category);
            let mut state = self.state.lock().map_err(|e| e.to_string())?;
            state.payloads = map;
            return Ok(());
        }

        let (plan, strategy, last_result) = {
            let state = self.state.lock().map_err(|e| e.to_string())?;
            (
                state.plan.clone(),
                state.strategy.clone(),
                state.last_result.clone(),
            )
        };

        let feedback = if strategy.enable_response_adaptation || self.config.adaptive_planning {
            last_result.as_ref().map(|result| {
                let judged: Vec<(bool, f32, &str)> = result
                    .judged
                    .iter()
                    .map(|j| (j.vulnerable, j.confidence, j.summary.as_str()))
                    .collect();
                let mut text = promptlab_generator::feedback_from_judged(&judged).unwrap_or_else(|| {
                    format!(
                        "attempt {} inconclusive: {} successes / {} attempts",
                        attempt - 1,
                        result.successes,
                        result.attempts
                    )
                });
                if !focus_hints.is_empty() {
                    text.push_str(&format!(" | focus_hints: {}", focus_hints.join("; ")));
                }
                text
            })
        } else {
            None
        };

        let retry = attempt.saturating_sub(1);
        let payloads = regenerate_category_payloads(
            self.ctx.data_dir,
            Arc::clone(&self.ctx.inference_manager),
            Arc::clone(&self.ctx.model_manager),
            self.ctx.model_provider.clone(),
            Arc::clone(&self.ctx.runtime_manager),
            &plan,
            self.category,
            &strategy,
            self.ctx.profile,
            self.catalog.clone(),
            feedback,
            retry,
        )
        .await?;

        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        state.payloads = payloads;
        Ok(())
    }

    async fn run_attack_attempt(
        &self,
        attempt: u32,
    ) -> Result<AttackAttemptObservation, String> {
        let (payloads, strategy, plan, pacing) = {
            let state = self.state.lock().map_err(|e| e.to_string())?;
            (
                state.payloads.clone(),
                state.strategy.clone(),
                state.plan.clone(),
                state.pacing.clone(),
            )
        };

        let attempt_options = CategoryRunOptions::from_strategy(
            self.category,
            &plan.disabled_tests,
            &strategy,
        )
        .with_pacing(&pacing);
        let owned_options = if attempt <= 1 {
            merge_run_options_with_pacing(
                self.initial_run_options,
                &pacing,
                self.category,
                &plan.disabled_tests,
                Some(&strategy),
            )
        } else {
            attempt_options
        };

        let result = run_category_on_target_profile(
            self.ctx.repos,
            self.ctx.scan_id,
            self.ctx.project_id,
            self.ctx.target_id,
            self.ctx.profile,
            self.category,
            self.ctx.attack_runtime.clone(),
            self.ctx.data_dir,
            Arc::clone(&self.ctx.inference_manager),
            Arc::clone(&self.ctx.model_manager),
            self.ctx.model_provider.clone(),
            Arc::clone(&self.ctx.runtime_manager),
            self.ctx.plugin_manager.clone(),
            Some(&payloads),
            Some(&self.ctx.emitter),
            Some(&owned_options),
            Some(&self.ctx.progress),
            self.ctx.job_controls.as_ref(),
        )
        .await
        .map_err(|err| err.to_string())?;

        if category_result_produced_no_requests(&result) {
            return Err(format!(
                "attack produced no requests for {}",
                self.category.as_str()
            ));
        }

        let obs = observation_from_category_result(&result);
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        state.last_result = Some(result);
        Ok(obs)
    }

    async fn apply_adapt(&self, adapt: &AdaptPlanOutcome) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        let base_plan = state.plan.clone();
        let base_strategy = state.strategy.clone();

        let mut next_strategy = base_strategy.clone();
        if adapt.escalate_mutation {
            next_strategy.mutation_level = next_strategy.mutation_level.escalate();
        }
        if adapt.escalate_strategy {
            next_strategy.strategy = next_strategy.strategy.escalate();
        }
        if adapt.increase_variants {
            next_strategy.variants_per_test =
                (next_strategy.variants_per_test.saturating_add(2)).min(20);
        }
        if adapt.enable_response_adaptation {
            next_strategy.enable_response_adaptation = true;
            next_strategy.enable_context_awareness = true;
        }
        next_strategy = next_strategy.clamp();

        let mut next_plan = base_plan.clone();
        for id in &adapt.disable_technique_ids {
            if !next_plan.disabled_tests.iter().any(|d| d == id) {
                next_plan.disabled_tests.push(id.clone());
            }
        }

        if let Some(ref last) = state.last_result {
            // Technique rotation only — strategy escalation already applied from AttackPlanAgent.
            let (rotated, _, rot_notes) = adapt_plan_for_retry(
                &next_plan,
                self.category,
                &next_strategy,
                last,
                self.catalog,
            );
            next_plan = rotated;
            for note in rot_notes {
                self.ctx.emitter.info(format!("adapt: {note}"));
            }
        }

        state.plan = next_plan;
        state.strategy = next_strategy;

        for note in &adapt.notes {
            self.ctx
                .emitter
                .info(format!("AttackPlanAgent adapt: {note}"));
        }

        Ok(())
    }

    async fn current_pacing(&self) -> EndpointPacing {
        self.state
            .lock()
            .map(|s| s.pacing.clone())
            .unwrap_or_default()
    }

    async fn apply_pacing(&self, pacing: &EndpointPacing) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        state.pacing = pacing.clone();
        self.ctx
            .emitter
            .info(format!("Agentic pacing updated: {}", pacing.summary()));
        Ok(())
    }

    async fn wait_backoff(&self, delay_ms: u64) {
        if delay_ms == 0 {
            return;
        }
        self.ctx.emitter.info(format!(
            "Agentic endpoint backoff waiting {delay_ms}ms before retry"
        ));
        tokio::time::sleep(Duration::from_millis(delay_ms.min(60_000))).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use promptlab_attack::AttackCategory;
    use promptlab_target_profile::{PayloadGenerationStrategy, PayloadStrategy};

    fn sample_strategy() -> PayloadStrategy {
        PayloadStrategy {
            strategy: PayloadGenerationStrategy::Deterministic,
            ..PayloadStrategy::default()
        }
    }

    #[test]
    fn adaptive_plan_rotates_failed_techniques_and_escalates_strategy() {
        let catalog = promptlab_payload::PayloadDatabase::builtin().expect("catalog");
        let plan = AttackPlan {
            mode: promptlab_planner::PlannerMode::Deterministic,
            profile_id: "standard".into(),
            categories: vec![AttackCategory::PromptInjection],
            disabled_tests: vec![],
            rationales: vec![],
            confidence: 0.8,
            summary: "test".into(),
            llm_rationale: None,
        };
        let strategy = sample_strategy();
        let last = CategoryRunResult {
            attempts: 1,
            successes: 0,
            findings: vec![],
            judged: vec![JudgedAttemptSummary {
                payload_id: "pi-direct-override".into(),
                payload_name: "Direct".into(),
                vulnerable: false,
                confidence: 0.1,
                summary: "refused".into(),
            }],
            ..Default::default()
        };
        let (next_plan, next_strategy, notes) = adapt_plan_for_retry(
            &plan,
            AttackCategory::PromptInjection,
            &strategy,
            &last,
            &catalog,
        );
        assert!(next_plan.disabled_tests.iter().any(|id| id == "pi-direct-override"));
        assert_ne!(next_strategy.mutation_level, strategy.mutation_level);
        assert!(next_strategy.enable_response_adaptation);
        assert!(!notes.is_empty());
    }

    #[test]
    fn sequential_progress_total_includes_pipeline_phases() {
        let categories = vec![AttackCategory::PromptInjection];
        let config = ScanExecutionConfig::from_flags(false, 1, false, false, Some(sample_strategy()), None);
        let attack_units = estimate_scan_requests(
            &categories,
            &[],
            &sample_strategy(),
            ExecutionStrategy::Sequential,
            1,
        ) as u64;
        let total = scan_progress_total(&categories, &[], &config);
        assert_eq!(total, 2 + attack_units * 2);
    }

    #[test]
    fn agentic_progress_total_includes_generate_reflection_and_retry() {
        let categories = vec![AttackCategory::PromptInjection];
        let config = ScanExecutionConfig::from_flags(true, 3, true, false, Some(sample_strategy()), None);
        let attack_units = estimate_scan_requests(
            &categories,
            &[],
            &sample_strategy(),
            ExecutionStrategy::Agentic,
            3,
        ) as u64;
        let total = scan_progress_total(&categories, &[], &config);
        // 1 prepare + 3 generate + 2*attack + 3 reflection + 2 retry
        assert_eq!(total, 1 + 3 + attack_units * 2 + 3 + 2);
    }

    #[test]
    fn agentic_progress_total_includes_adaptive_units() {
        let categories = vec![AttackCategory::PromptInjection];
        let config = ScanExecutionConfig::from_flags(true, 3, true, true, Some(sample_strategy()), None);
        let attack_units = estimate_scan_requests(
            &categories,
            &[],
            &sample_strategy(),
            ExecutionStrategy::Agentic,
            3,
        ) as u64;
        let total = scan_progress_total(&categories, &[], &config);
        // 1 prepare + 3 generate + 2*attack + 3 reflection + 2 adaptive + 2 retry
        assert_eq!(total, 1 + 3 + attack_units * 2 + 3 + 2 + 2);
    }
}
