//! Attack execution commands.
//!
//! `attack_run_prompt_injection` executes the real `aisec-attack` prompt
//! injection attack (harness transport + built-in evaluation) against a previously
//! discovered endpoint, persists every attempt as an `attack_result` and every
//! successful attempt as a `finding`, and returns the run summary. No mocked
//! findings: results come straight from the engine evaluating real target
//! responses normalized by the harness layer.

use aisec_attack::{
    apply_descriptor_auth, AttackCategory, AttackContext, AttackPayload, AttackTarget,
    AttackExecutor, FindingSeverity, PayloadAttempt, DEFAULT_ATTACK_CONCURRENCY,
};
use aisec_endpoint_metadata::body_template_from_metadata;
use crate::dto::metadata_from_endpoint;
use aisec_target_profile::{TargetProfile, TargetProvider};
use aisec_auth::{resolve_descriptor_for_runtime, resolve_descriptor_for_wizard, AuthSessionManager, SecretStore};
use aisec_judge::{JudgeVerdict, Severity as JudgeSeverity};
use aisec_plugin_host::evaluate_with_judge_plugins;
use aisec_inference::InferenceRuntimeManager;
use aisec_runtime::{RuntimeManager, SharedModelProvider};
use aisec_storage::{
    AttackResultRepository, CreateAttackResult, CreateFinding, CreateScan, Endpoint,
    EndpointRepository, FindingRepository, Repositories, ScanRepository, TargetRepository,
    UpdateScan,
};
use tauri::State;
use time::OffsetDateTime;
use tracing::{info, instrument, warn};

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::mpsc;

use tauri::async_runtime::Mutex as AsyncMutex;

use crate::dto::{AttackRunDto, FindingDto, ScanDto};
use crate::error::{CommandError, CommandResult};
use crate::events::{ScanProgressEmitter, ScanProgressLevel};
use crate::inference_host::build_judge_engine_from_gateway;
use crate::session_auth::{attack_executor, attack_executor_with_variants, build_attack_runtime, AttackRuntime};
use crate::jobs::{bump_scan_progress, ScanBatchCheckpoint, ScanJobControls, ScanProgress};
use crate::scan_playbook::persist_scan_playbook_state;
use crate::state::AppState;

pub struct CategoryRunOptions {
    pub max_payloads: usize,
    pub variants_per_test: usize,
}

impl CategoryRunOptions {
    pub fn from_strategy(
        category: AttackCategory,
        disabled_tests: &[String],
        strategy: &aisec_target_profile::PayloadStrategy,
    ) -> Self {
        let enabled =
            aisec_target_profile::wizard_plan::enabled_tests_for_category(category, disabled_tests)
                as usize;
        let variants = strategy.variants_per_test.max(1) as usize;
        let budget = strategy.max_total_payloads.max(1) as usize;
        Self {
            max_payloads: enabled.saturating_mul(variants).saturating_mul(budget),
            variants_per_test: variants,
        }
    }
}

fn attack_executor_for_options(
    transport: crate::plugin_transport::PluginAwareTransport,
    options: &CategoryRunOptions,
) -> AttackExecutor<crate::plugin_transport::PluginAwareTransport> {
    if options.variants_per_test <= 1 {
        attack_executor(transport)
    } else {
        attack_executor_with_variants(transport, options.variants_per_test)
    }
}

fn update_scan_phase(
    progress: Option<&Arc<Mutex<ScanProgress>>>,
    phase: &str,
    test: Option<&str>,
    attempt: Option<u32>,
    retry: Option<u32>,
) {
    let Some(progress) = progress else {
        return;
    };
    if let Ok(mut state) = progress.lock() {
        state.current_phase = Some(phase.into());
        if let Some(label) = test {
            state.current_test = Some(label.into());
        }
        state.current_attempt = attempt;
        state.current_retry = retry;
    }
}

#[derive(Clone)]
pub struct JudgedAttemptSummary {
    pub payload_id: String,
    pub payload_name: String,
    pub vulnerable: bool,
    pub confidence: f32,
    pub summary: String,
}

pub struct CategoryRunResult {
    pub attempts: usize,
    pub successes: u64,
    pub findings: Vec<FindingDto>,
    pub judged: Vec<JudgedAttemptSummary>,
}

/// Apply descriptor auth without aborting the scan when keychain entries are missing.
/// Wizard verification may use inline headers while the DB descriptor only stores credential refs.
fn merge_descriptor_auth_target(
    target: AttackTarget,
    descriptor_json: &str,
    target_id: &str,
) -> CommandResult<AttackTarget> {
    let secrets = SecretStore::new().map_err(CommandError::from)?;
    let resolved = match resolve_descriptor_for_runtime(descriptor_json, &secrets) {
        Ok(resolved) => resolved,
        Err(err) => {
            warn!(
                target_id = %target_id,
                error = %err,
                "descriptor credentials missing from vault; falling back to profile/session auth"
            );
            resolve_descriptor_for_wizard(descriptor_json, &secrets)
                .unwrap_or_else(|_| descriptor_json.to_string())
        }
    };
    Ok(apply_descriptor_auth(target, &resolved))
}

fn harness_surface_for_provider(provider: TargetProvider) -> &'static str {
    match provider {
        TargetProvider::GenericHttp
        | TargetProvider::Mcp
        | TargetProvider::Dify
        | TargetProvider::Langflow => "rest_api",
        TargetProvider::AnthropicClaude => "anthropic_compatible",
        _ => "openai_compatible",
    }
}

fn finding_evidence_json(
    url: &str,
    method: Option<&str>,
    body_template: Option<&str>,
    attempt: &PayloadAttempt,
    verdict_summary: &str,
    verdict_confidence: f32,
    indicators: &[String],
    judge_json: &serde_json::Value,
    provider: Option<&str>,
) -> serde_json::Value {
    let mut value = serde_json::json!({
        "payload_id": attempt.payload_id,
        "payload": attempt.mutated_content,
        "request": {
            "url": url,
            "method": method.unwrap_or("POST"),
            "body": attempt.mutated_content,
            "body_template": body_template,
        },
        "response": {
            "status": attempt.response.status,
            "body": attempt.response.body,
            "duration_ms": attempt.response.duration_ms,
            "normalized": attempt.response.normalized.content,
        },
        "response_excerpt": attempt.response.body.chars().take(2000).collect::<String>(),
        "explanation": verdict_summary,
        "verdict": "vulnerable",
        "confidence": verdict_confidence,
        "indicators": indicators,
        "judge": judge_json,
    });
    if let Some(provider) = provider {
        if let Some(obj) = value.as_object_mut() {
            obj.insert("provider".into(), serde_json::json!(provider));
        }
    }
    value
}

fn severity_str(severity: FindingSeverity) -> &'static str {
    match severity {
        FindingSeverity::Info => "info",
        FindingSeverity::Low => "low",
        FindingSeverity::Medium => "medium",
        FindingSeverity::High => "high",
        FindingSeverity::Critical => "critical",
    }
}

fn judge_severity_str(severity: JudgeSeverity) -> &'static str {
    match severity {
        JudgeSeverity::Info => "info",
        JudgeSeverity::Low => "low",
        JudgeSeverity::Medium => "medium",
        JudgeSeverity::High => "high",
        JudgeSeverity::Critical => "critical",
    }
}

pub fn category_id(category: AttackCategory) -> &'static str {
    category.as_str()
}

/// Build judge engine while briefly holding inference/runtime locks (released before HTTP-heavy work).
async fn build_judge_for_category(
    data_dir: &std::path::Path,
    inference_manager: Arc<AsyncMutex<InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<aisec_models::LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
) -> CommandResult<aisec_judge::JudgeEngine> {
    let inference = inference_manager.lock().await;
    let manager = model_manager.lock().await;
    let mut runtime_mgr = runtime_manager.lock().await;
    build_judge_engine_from_gateway(
        data_dir,
        &inference,
        &manager,
        model_provider,
        &mut runtime_mgr,
    )
    .await
}

struct CategoryJudgeAccum {
    successes: u64,
    created_findings: Vec<FindingDto>,
    judged: Vec<JudgedAttemptSummary>,
}

struct CategoryJudgeEnv<'a> {
    repos: &'a Repositories,
    scan_id: &'a str,
    project_id: &'a str,
    target_id: Option<String>,
    category: AttackCategory,
    category_name: &'a str,
    endpoint_url: &'a str,
    method: Option<&'a str>,
    body_template: Option<&'a str>,
    provider: Option<&'a str>,
    judge: &'a aisec_judge::JudgeEngine,
    plugin_manager: &'a Arc<AsyncMutex<aisec_plugin_host::PluginManager>>,
    progress: Option<&'a ScanProgressEmitter>,
    progress_state: Option<&'a Arc<Mutex<ScanProgress>>>,
    job_controls: Option<&'a ScanJobControls>,
}

async fn judge_single_attempt(
    env: &CategoryJudgeEnv<'_>,
    seq: usize,
    attempt: PayloadAttempt,
    accum: &mut CategoryJudgeAccum,
) -> CommandResult<()> {
    let eval = &attempt.evaluation;
    let normalized = &attempt.response.normalized;

    let mut verdict: JudgeVerdict = env
        .judge
        .judge_normalized(
            attempt.payload_id.clone(),
            env.category_name,
            attempt.mutated_content.clone(),
            normalized,
        )
        .await
        .map_err(|err| CommandError::from(aisec_core::AisecError::internal(err.to_string())))?;

    {
        let mut plugins = env.plugin_manager.lock().await;
        if let Ok(signals) = evaluate_with_judge_plugins(
            &mut plugins,
            &attempt.response.body,
            env.category_name,
        )
        .await
        {
            for signal in signals {
                if signal.vulnerable
                    && (signal.confidence > verdict.confidence || !verdict.vulnerable)
                {
                    verdict.vulnerable = true;
                    verdict.confidence = verdict.confidence.max(signal.confidence);
                    verdict.summary = format!(
                        "{} [plugin {}: {}]",
                        verdict.summary, signal.plugin_id, signal.summary
                    );
                }
            }
        }
    }

    let judge_json = serde_json::to_value(&verdict).unwrap_or(serde_json::Value::Null);

    env.repos
        .attack_results()
        .create(CreateAttackResult {
            scan_id: env.scan_id.to_string(),
            payload_id: None,
            target_id: env.target_id.clone(),
            probe_id: Some(attempt.payload_id.clone()),
            success: verdict.vulnerable,
            response_json: Some(serde_json::json!({
                "status": attempt.response.status,
                "body": attempt.response.body,
                "duration_ms": attempt.response.duration_ms,
                "normalized": normalized,
            })),
            evaluated_json: Some(serde_json::json!({
                "attack_evaluation": {
                    "success": eval.success,
                    "confidence": eval.confidence,
                    "severity": eval.severity.map(severity_str),
                    "indicators": eval.indicators,
                    "summary": eval.summary,
                },
                "judge": judge_json,
            })),
            duration_ms: Some(attempt.response.duration_ms as i64),
        })
        .await
        .map_err(CommandError::from)?;

    if verdict.vulnerable {
        accum.successes += 1;
        let severity = verdict
            .severity
            .map(judge_severity_str)
            .or_else(|| eval.severity.map(severity_str))
            .unwrap_or("medium");
        let finding = env
            .repos
            .findings()
            .create(CreateFinding {
                scan_id: env.scan_id.to_string(),
                project_id: env.project_id.to_string(),
                target_id: env.target_id.clone(),
                title: format!("{}: {}", env.category.display_name(), attempt.payload_name),
                severity: severity.to_string(),
                category: Some(env.category_name.into()),
                description: Some(verdict.summary.clone()),
                evidence_json: Some(finding_evidence_json(
                    env.endpoint_url,
                    env.method,
                    env.body_template,
                    &attempt,
                    &verdict.summary,
                    verdict.confidence,
                    &eval.indicators,
                    &judge_json,
                    env.provider,
                )),
                status: None,
            })
            .await
            .map_err(CommandError::from)?;
        let finding_dto = FindingDto::from(finding);
        accum.created_findings.push(finding_dto.clone());
        if let Some(emitter) = env.progress {
            emitter.detailed(
                ScanProgressLevel::Info,
                emitter
                    .event(ScanProgressLevel::Info, "Saved finding")
                    .endpoint(env.endpoint_url)
                    .finding_id(&finding_dto.id),
            );
        }
    }

    accum.judged.push(JudgedAttemptSummary {
        payload_id: attempt.payload_id.clone(),
        payload_name: attempt.payload_name.clone(),
        vulnerable: verdict.vulnerable,
        confidence: verdict.confidence,
        summary: verdict.summary.clone(),
    });
    if let Some(progress_state) = env.progress_state {
        bump_scan_progress(progress_state, 1);
    }

    if let Some(emitter) = env.progress {
        let confidence_label = if verdict.confidence >= 0.8 {
            "High Confidence"
        } else if verdict.confidence >= 0.5 {
            "Medium Confidence"
        } else {
            "Low Confidence"
        };
        emitter.detailed(
            ScanProgressLevel::Info,
            emitter
                .event(ScanProgressLevel::Info, format!("Judge: {confidence_label}"))
                .endpoint(env.endpoint_url)
                .payload(&truncate_console_payload(&attempt.mutated_content, 500)),
        );
    }

    Ok(())
}

fn truncate_console_payload(content: &str, max_bytes: usize) -> String {
    let trimmed = content.trim();
    if trimmed.len() <= max_bytes {
        return trimmed.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &trimmed[..end])
}

fn emit_attack_attempt(
    emitter: &ScanProgressEmitter,
    endpoint_url: &str,
    seq: usize,
    attempt: &PayloadAttempt,
) {
    emitter.detailed(
        ScanProgressLevel::Info,
        emitter
            .event(
                ScanProgressLevel::Info,
                format!("Attack #{} {}", seq + 1, attempt.payload_name),
            )
            .endpoint(endpoint_url)
            .payload(&truncate_console_payload(&attempt.mutated_content, 500)),
    );
    emitter.detailed(
        ScanProgressLevel::Info,
        emitter
            .event(
                ScanProgressLevel::Info,
                format!(
                    "Response {} ({}ms)",
                    attempt.response.status, attempt.response.duration_ms
                ),
            )
            .endpoint(endpoint_url)
            .status_code(attempt.response.status)
            .latency(attempt.response.duration_ms),
    );
}

async fn collect_category_attempts(
    executor: &AttackExecutor<crate::plugin_transport::PluginAwareTransport>,
    category: AttackCategory,
    ctx: &AttackContext,
    emitter: Option<&ScanProgressEmitter>,
    endpoint_url: &str,
    progress_state: Option<&Arc<Mutex<ScanProgress>>>,
) -> CommandResult<Vec<PayloadAttempt>> {
    use std::collections::BTreeMap;

    let (tx, mut rx) = mpsc::channel(DEFAULT_ATTACK_CONCURRENCY.max(4));
    let mut exec = std::pin::pin!(executor.execute_category_streaming(category, &ctx, tx));

    let mut ordered: BTreeMap<usize, PayloadAttempt> = BTreeMap::new();
    let mut next_seq = 0usize;
    let mut attempts = Vec::new();

    loop {
        tokio::select! {
            result = exec.as_mut() => {
                result.map_err(|err| CommandError::from(aisec_core::AisecError::internal(err.to_string())))?;
                break;
            }
            item = rx.recv() => {
                match item {
                    Some((seq, attempt)) => {
                        ordered.insert(seq, attempt);
                        while let Some(attempt) = ordered.remove(&next_seq) {
                            if let Some(emitter) = emitter {
                                emit_attack_attempt(emitter, endpoint_url, next_seq, &attempt);
                            }
                            if let Some(progress_state) = progress_state {
                                bump_scan_progress(progress_state, 1);
                                if let Ok(mut state) = progress_state.lock() {
                                    state.attacks_completed = state
                                        .attacks_completed
                                        .saturating_add(1)
                                        .min(state.attacks_total.max(1));
                                    state.sync_testcases_completed();
                                }
                            }
                            attempts.push(attempt);
                            next_seq += 1;
                        }
                    }
                    None => break,
                }
            }
        }
    }

    while let Some(attempt) = ordered.remove(&next_seq) {
        if let Some(emitter) = emitter {
            emit_attack_attempt(emitter, endpoint_url, next_seq, &attempt);
        }
        if let Some(progress_state) = progress_state {
            bump_scan_progress(progress_state, 1);
            if let Ok(mut state) = progress_state.lock() {
                state.attacks_completed = state
                    .attacks_completed
                    .saturating_add(1)
                    .min(state.attacks_total.max(1));
                state.sync_testcases_completed();
            }
        }
        attempts.push(attempt);
        next_seq += 1;
    }

    Ok(attempts)
}

async fn wait_if_job_paused(controls: &ScanJobControls) {
    while controls.paused.load(Ordering::Relaxed) {
        if controls.cancel.load(Ordering::Relaxed) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn park_at_batch_checkpoint(
    controls: &ScanJobControls,
    checkpoint: ScanBatchCheckpoint,
    repos: &Repositories,
    scan_id: &str,
    progress_state: Option<&Arc<Mutex<ScanProgress>>>,
    emitter: Option<&ScanProgressEmitter>,
    category_label: &str,
) {
    let checkpoint_for_db = checkpoint.clone();
    {
        *controls.batch_checkpoint.lock().unwrap() = Some(checkpoint);
    }
    controls.pause_requested.store(false, Ordering::Relaxed);
    controls.paused.store(true, Ordering::Relaxed);

    let progress_snapshot = if let Some(progress_state) = progress_state {
        if let Ok(mut progress) = progress_state.lock() {
            progress.status = "paused".into();
            progress.pause_pending = false;
            progress.current_test = Some(category_label.into());
            Some(progress.clone())
        } else {
            None
        }
    } else {
        None
    };

    let _ = repos
        .scans()
        .update(
            scan_id,
            UpdateScan {
                status: Some("paused".into()),
                ..Default::default()
            },
        )
        .await;

    if let Some(progress) = progress_snapshot.as_ref() {
        let _ = persist_scan_playbook_state(
            repos,
            scan_id,
            Some(progress),
            Some(Some(&checkpoint_for_db)),
        )
        .await;
    } else {
        let _ = persist_scan_playbook_state(repos, scan_id, None, Some(Some(&checkpoint_for_db))).await;
    }

    if let Some(emitter) = emitter {
        emitter.info(format!(
            "Paused at batch checkpoint for {category_label} — press Resume to continue"
        ));
    }

    wait_if_job_paused(controls).await;
}

async fn execute_category_then_judge(
    executor: AttackExecutor<crate::plugin_transport::PluginAwareTransport>,
    category: AttackCategory,
    ctx: AttackContext,
    env: &CategoryJudgeEnv<'_>,
) -> CommandResult<CategoryRunResult> {
    let category_label = env.category.display_name();
    let mut accum = CategoryJudgeAccum {
        successes: 0,
        created_findings: Vec::new(),
        judged: Vec::new(),
    };

    'category: loop {
        let (attempts, judge_start) = if let Some(ctrl) = env.job_controls {
            let restored_checkpoint = {
                let mut guard = ctrl.batch_checkpoint.lock().unwrap();
                guard.take()
            };
            if let Some(checkpoint) = restored_checkpoint {
                match checkpoint {
                    ScanBatchCheckpoint::PendingJudge { attempts, .. } => (attempts, 0usize),
                    ScanBatchCheckpoint::JudgingPartial {
                        attempts,
                        next_judge_index,
                        ..
                    } => (attempts, next_judge_index),
                }
            } else {
                let collected = collect_category_attempts(
                    &executor,
                    category,
                    &ctx,
                    env.progress,
                    env.endpoint_url,
                    env.progress_state,
                )
                .await?;
                if ctrl.cancel.load(Ordering::Relaxed) {
                    return Ok(category_result_from_accum(&accum));
                }
                if ctrl.pause_requested.load(Ordering::Relaxed) {
                    park_at_batch_checkpoint(
                        ctrl,
                        ScanBatchCheckpoint::PendingJudge {
                            category: category_label.to_string(),
                            attempts: collected.clone(),
                        },
                        env.repos,
                        env.scan_id,
                        env.progress_state,
                        env.progress,
                        &category_label,
                    )
                    .await;
                    continue 'category;
                }
                (collected, 0)
            }
        } else {
            (
                collect_category_attempts(
                    &executor,
                    category,
                    &ctx,
                    env.progress,
                    env.endpoint_url,
                    env.progress_state,
                )
                .await?,
                0,
            )
        };

        if attempts.is_empty() {
            return Ok(category_result_from_accum(&accum));
        }

        if judge_start == 0 {
            if let Some(emitter) = env.progress {
                emitter.info(format!(
                    "Judging {} attack{} for {}",
                    attempts.len(),
                    if attempts.len() == 1 { "" } else { "s" },
                    category_label,
                ));
            }
            update_scan_phase(
                env.progress_state,
                "judge",
                Some(&category_label),
                None,
                None,
            );
        }

        let mut paused_mid_judge = false;
        for seq in judge_start..attempts.len() {
            if let Some(ctrl) = env.job_controls {
                wait_if_job_paused(ctrl).await;
                if ctrl.cancel.load(Ordering::Relaxed) {
                    return Ok(category_result_from_accum(&accum));
                }
            }

            judge_single_attempt(env, seq, attempts[seq].clone(), &mut accum).await?;

            if let Some(ctrl) = env.job_controls {
                if ctrl.pause_requested.load(Ordering::Relaxed) {
                    park_at_batch_checkpoint(
                        ctrl,
                        ScanBatchCheckpoint::JudgingPartial {
                            category: category_label.to_string(),
                            attempts: attempts.clone(),
                            next_judge_index: seq + 1,
                        },
                        env.repos,
                        env.scan_id,
                        env.progress_state,
                        env.progress,
                        &category_label,
                    )
                    .await;
                    paused_mid_judge = true;
                    break;
                }
            }
        }

        if paused_mid_judge {
            continue 'category;
        }

        if let Some(ctrl) = env.job_controls {
            if ctrl.paused.load(Ordering::Relaxed) {
                continue 'category;
            }
        }
        break 'category;
    }

    if let Some(ctrl) = env.job_controls {
        {
            let mut guard = ctrl.batch_checkpoint.lock().unwrap();
            *guard = None;
        }
        let _ = persist_scan_playbook_state(env.repos, env.scan_id, None, Some(None)).await;
    }

    Ok(category_result_from_accum(&accum))
}

fn category_result_from_accum(accum: &CategoryJudgeAccum) -> CategoryRunResult {
    CategoryRunResult {
        attempts: accum.judged.len(),
        successes: accum.successes,
        findings: accum.created_findings.clone(),
        judged: accum.judged.clone(),
    }
}

/// Execute one attack category against a single endpoint; persist attempts and findings.
pub async fn run_category_on_endpoint(
    repos: &Repositories,
    scan_id: &str,
    project_id: &str,
    target_id: Option<String>,
    endpoint: &Endpoint,
    category: AttackCategory,
    runtime: AttackRuntime,
    data_dir: &std::path::Path,
    inference_manager: Arc<AsyncMutex<InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<aisec_models::LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    plugin_manager: Arc<AsyncMutex<aisec_plugin_host::PluginManager>>,
    generated_payloads: Option<&HashMap<AttackCategory, Vec<AttackPayload>>>,
    progress: Option<&ScanProgressEmitter>,
    options: Option<&CategoryRunOptions>,
    progress_state: Option<&Arc<Mutex<ScanProgress>>>,
) -> CommandResult<CategoryRunResult> {
    let mut target = AttackTarget::llm_api(endpoint.url.clone());
    if let Some(method) = &endpoint.method {
        target.method = Some(method.clone());
    }
    if let Some(metadata) = metadata_from_endpoint(endpoint) {
        target.body_template = Some(body_template_from_metadata(&metadata));
    }
    if let Some(tid) = &target_id {
        if let Ok(stored_target) = repos.targets().get(tid).await {
            target = merge_descriptor_auth_target(target, &stored_target.descriptor_json, tid)?;
        }
    }

    if let Some(ctx) = &runtime.session {
        let mut headers = AuthSessionManager::auth_headers(ctx);
        if let Some(cookie) = AuthSessionManager::cookie_header_for_url(ctx, &endpoint.url) {
            headers.insert("Cookie".into(), cookie);
        }
        for (key, value) in headers {
            target = target.with_header(&key, value);
        }
    }

    let probe_id = format!("{}-{}", endpoint.id, category.as_str());
    let mut ctx = AttackContext::new(scan_id, probe_id, target);
    ctx.target_id = target_id.clone();
    if let Some(payloads) = generated_payloads {
        ctx = ctx.with_generated_payloads(payloads.clone());
    }
    if let Some(opts) = options {
        ctx.budget.max_payloads = opts.max_payloads;
    }
    ctx.budget.max_concurrent_requests = DEFAULT_ATTACK_CONCURRENCY;
    let executor = if let Some(opts) = options {
        attack_executor_for_options(runtime.transport, opts)
    } else {
        attack_executor(runtime.transport)
    };

    info!(
        scan_id = %scan_id,
        endpoint_id = %endpoint.id,
        category = %category.as_str(),
        url = %endpoint.url,
        method = ?endpoint.method,
        "attack unit started"
    );

    let method_label = endpoint
        .method
        .as_deref()
        .unwrap_or("POST");
    if let Some(emitter) = progress {
        let path = url::Url::parse(&endpoint.url)
            .ok()
            .map(|u| u.path().to_string())
            .unwrap_or_else(|| endpoint.url.clone());
        emitter.info(format!("Testing {method_label} {path}"));
    }

    update_scan_phase(
        progress_state,
        "attack",
        Some(category.display_name()),
        None,
        None,
    );

    let judge = build_judge_for_category(
        data_dir,
        inference_manager,
        model_manager,
        model_provider.clone(),
        runtime_manager,
    )
    .await?;
    let endpoint_metadata = metadata_from_endpoint(endpoint);
    let body_template = endpoint_metadata
        .as_ref()
        .map(|meta| body_template_from_metadata(meta));
    let judge_env = CategoryJudgeEnv {
        repos,
        scan_id,
        project_id,
        target_id: target_id.clone(),
        category,
        category_name: category.as_str(),
        endpoint_url: &endpoint.url,
        method: endpoint.method.as_deref(),
        body_template: body_template.as_deref(),
        provider: None,
        judge: &judge,
        plugin_manager: &plugin_manager,
        progress,
        progress_state,
        job_controls: None,
    };

    execute_category_then_judge(executor, category, ctx, &judge_env).await
}

/// Execute one attack category against a verified AI Target Profile.
pub async fn run_category_on_target_profile(
    repos: &Repositories,
    scan_id: &str,
    project_id: &str,
    target_id: &str,
    profile: &TargetProfile,
    category: AttackCategory,
    runtime: AttackRuntime,
    data_dir: &std::path::Path,
    inference_manager: Arc<AsyncMutex<InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<aisec_models::LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    plugin_manager: Arc<AsyncMutex<aisec_plugin_host::PluginManager>>,
    generated_payloads: Option<&HashMap<AttackCategory, Vec<AttackPayload>>>,
    progress: Option<&ScanProgressEmitter>,
    options: Option<&CategoryRunOptions>,
    progress_state: Option<&Arc<Mutex<ScanProgress>>>,
    job_controls: Option<&ScanJobControls>,
) -> CommandResult<CategoryRunResult> {
    let url = profile.full_url();
    let mut target = AttackTarget::llm_api(url.clone());
    target.method = Some(profile.method.as_str().into());
    target.body_template = Some(profile.request_template.clone());
    target.prompt_placeholder = Some(profile.prompt_placeholder.clone());
    target.harness_surface = Some(harness_surface_for_provider(profile.provider).into());
    for (key, value) in &profile.headers {
        target = target.with_header(key, value);
    }

    if let Ok(stored_target) = repos.targets().get(target_id).await {
        target = merge_descriptor_auth_target(target, &stored_target.descriptor_json, target_id)?;
    }

    if let Some(ctx) = &runtime.session {
        let mut headers = AuthSessionManager::auth_headers(ctx);
        if let Some(cookie) = AuthSessionManager::cookie_header_for_url(ctx, &url) {
            headers.insert("Cookie".into(), cookie);
        }
        for (key, value) in headers {
            target = target.with_header(&key, value);
        }
    }

    let probe_id = format!("{target_id}-{}", category.as_str());
    let mut ctx = AttackContext::new(scan_id, probe_id, target);
    ctx.target_id = Some(target_id.to_string());
    if let Some(payloads) = generated_payloads {
        ctx = ctx.with_generated_payloads(payloads.clone());
    }
    if let Some(opts) = options {
        ctx.budget.max_payloads = opts.max_payloads;
    }
    ctx.budget.max_concurrent_requests = DEFAULT_ATTACK_CONCURRENCY;
    let executor = if let Some(opts) = options {
        attack_executor_for_options(runtime.transport, opts)
    } else {
        attack_executor(runtime.transport)
    };

    info!(
        scan_id = %scan_id,
        target_id = %target_id,
        category = %category.as_str(),
        url = %url,
        provider = %profile.provider.as_str(),
        "attack unit started (target profile)"
    );

    if let Some(emitter) = progress {
        emitter.info(format!(
            "Testing {} {}{}",
            profile.method.as_str(),
            profile.base_url.trim_end_matches('/'),
            profile.path
        ));
    }

    update_scan_phase(
        progress_state,
        "attack",
        Some(category.display_name()),
        None,
        None,
    );

    let judge = build_judge_for_category(
        data_dir,
        inference_manager,
        model_manager,
        model_provider.clone(),
        runtime_manager,
    )
    .await?;
    let judge_env = CategoryJudgeEnv {
        repos,
        scan_id,
        project_id,
        target_id: Some(target_id.to_string()),
        category,
        category_name: category.as_str(),
        endpoint_url: &url,
        method: Some(profile.method.as_str()),
        body_template: Some(profile.request_template.as_str()),
        provider: Some(profile.provider.as_str()),
        judge: &judge,
        plugin_manager: &plugin_manager,
        progress,
        progress_state,
        job_controls,
    };

    execute_category_then_judge(executor, category, ctx, &judge_env).await
}

#[instrument(skip(state))]
pub async fn attack_run_prompt_injection_op(
    state: &AppState,
    endpoint_id: String,
) -> CommandResult<AttackRunDto> {
    let repos = state.repositories();

    let endpoint = repos
        .endpoints()
        .get(&endpoint_id)
        .await
        .map_err(CommandError::from)?;
    let source_scan = repos
        .scans()
        .get(&endpoint.scan_id)
        .await
        .map_err(CommandError::from)?;
    let project_id = source_scan.project_id.clone();
    let target_id = endpoint.target_id.clone().or(source_scan.target_id.clone());

    let descriptor_json = if let Some(tid) = &target_id {
        repos
            .targets()
            .get(tid)
            .await
            .map(|t| t.descriptor_json)
            .unwrap_or_default()
    } else {
        String::new()
    };
    let runtime = build_attack_runtime(state, &descriptor_json, &endpoint.url).await?;

    let scan = repos
        .scans()
        .create(CreateScan {
            project_id: project_id.clone(),
            target_id: target_id.clone(),
            name: format!("Prompt Injection: {}", endpoint.url),
            status: Some("running".into()),
            playbook_json: Some(serde_json::json!({
                "attack": "prompt_injection",
                "endpoint_id": endpoint.id,
                "endpoint_url": endpoint.url,
            })),
        })
        .await
        .map_err(CommandError::from)?;

    let _ = repos
        .scans()
        .update(
            &scan.id,
            UpdateScan {
                started_at: Some(Some(OffsetDateTime::now_utc())),
                ..Default::default()
            },
        )
        .await;

    let category = AttackCategory::PromptInjection;
    let run = match run_category_on_endpoint(
        &repos,
        &scan.id,
        &project_id,
        target_id.clone(),
        &endpoint,
        category,
        runtime,
        state.data_dir(),
        Arc::clone(state.inference_manager()),
        Arc::clone(state.model_manager()),
        state.model_provider().clone(),
        Arc::clone(state.runtime_manager()),
        state.plugin_manager().clone(),
        None,
        None,
        None,
        None,
    )
    .await
    {
        Ok(run) => run,
        Err(err) => {
            warn!(scan_id = %scan.id, error = %err, "attack run failed");
            let _ = repos
                .scans()
                .update(
                    &scan.id,
                    UpdateScan {
                        status: Some("failed".into()),
                        completed_at: Some(Some(OffsetDateTime::now_utc())),
                        playbook_json: Some(serde_json::json!({ "error": err.to_string() })),
                        ..Default::default()
                    },
                )
                .await;
            return Err(err);
        }
    };

    let updated = repos
        .scans()
        .update(
            &scan.id,
            UpdateScan {
                status: Some("completed".into()),
                completed_at: Some(Some(OffsetDateTime::now_utc())),
                ..Default::default()
            },
        )
        .await
        .map_err(CommandError::from)?;

    info!(
        scan_id = %scan.id,
        attempts = run.attempts,
        successes = run.successes,
        findings = run.findings.len(),
        "prompt injection attack completed"
    );

    Ok(AttackRunDto {
        scan: ScanDto::from(updated),
        category: category_id(category).into(),
        attempts: run.attempts as u64,
        successes: run.successes,
        findings: run.findings,
    })
}

// ---------------------------------------------------------------------------
// Tauri command wrapper
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn attack_run_prompt_injection(
    state: State<'_, AppState>,
    endpoint_id: String,
) -> CommandResult<AttackRunDto> {
    attack_run_prompt_injection_op(state.inner(), endpoint_id).await
}

#[cfg(test)]
mod tests {
    use super::truncate_console_payload;

    #[test]
    fn truncate_console_payload_respects_char_boundaries() {
        let zwsp = "\u{200b}";
        let mut payload = "A".repeat(498);
        payload.push_str(zwsp);
        payload.push_str("tail");

        let truncated = truncate_console_payload(&payload, 500);
        assert!(truncated.ends_with('…'));
        assert!(truncated.len() <= 500 + '…'.len_utf8());
        assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
    }
}
