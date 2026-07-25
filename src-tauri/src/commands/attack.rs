//! Attack execution commands for scan jobs and category runs.

use promptlab_attack::{
    apply_descriptor_auth, AttackCategory, AttackContext, AttackPayload, AttackTarget,
    AttackExecutor, FindingSeverity, MutatorKind, PayloadAttempt, DEFAULT_ATTACK_CONCURRENCY,
};
use promptlab_endpoint_metadata::body_template_from_metadata;
use crate::dto::metadata_from_endpoint;
use promptlab_target_profile::{TargetProfile, TargetProvider};
use promptlab_auth::{resolve_descriptor_for_runtime, resolve_descriptor_for_wizard, AuthSessionManager, SecretStore};
use promptlab_agent::JudgeCoordinatorAgent;
use promptlab_judge::{JudgeRequest, JudgeVerdict, Severity as JudgeSeverity};
use promptlab_plugin_host::evaluate_with_judge_plugins;
use promptlab_inference::InferenceRuntimeManager;
use promptlab_runtime::{RuntimeManager, SharedModelProvider};
use promptlab_storage::{
    AttackResultRepository, CreateAttackResult, CreateFinding, Endpoint,
    EndpointRepository, FindingRepository, JudgeRoleWeightsRepository, Repositories,
    ScanRepository, TargetRepository, UpdateScan,
};
use time::OffsetDateTime;
use tracing::{info, instrument, warn};

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::mpsc;

use tauri::async_runtime::Mutex as AsyncMutex;

use crate::dto::FindingDto;
use crate::error::{CommandError, CommandResult};
use crate::events::{ScanProgressEmitter, ScanProgressLevel};
use crate::inference_host::build_judge_engine_from_gateway;
use crate::session_auth::{attack_executor_with_variants, build_attack_runtime, AttackRuntime};
use crate::jobs::{bump_scan_progress, ScanBatchCheckpoint, ScanJobControls, ScanProgress};
use crate::scan_playbook::persist_scan_playbook_state;
use crate::state::AppState;

#[derive(Clone)]
pub struct CategoryRunOptions {
    pub max_payloads: usize,
    pub variants_per_test: usize,
    pub max_concurrent_requests: Option<usize>,
    pub inter_request_delay_ms: Option<u64>,
    pub timeout_ms: Option<u64>,
    /// Attack-time mutator allowlist from payload strategy.
    /// `None` = category defaults; `Some([])` = no expand.
    pub enabled_mutators: Option<Vec<String>>,
}

impl CategoryRunOptions {
    /// Model A: HTTP work-item cap ≈ enabled_tests × variants_per_payload × payloads_per_testcase.
    pub fn from_strategy(
        category: AttackCategory,
        disabled_tests: &[String],
        strategy: &promptlab_target_profile::PayloadStrategy,
    ) -> Self {
        let enabled =
            promptlab_target_profile::wizard_plan::enabled_tests_for_category(category, disabled_tests)
                as usize;
        let variants = strategy.variants_per_test.max(1) as usize;
        let budget = strategy.max_total_payloads.max(1) as usize;
        Self {
            max_payloads: enabled
                .saturating_mul(variants)
                .saturating_mul(budget)
                .max(1),
            variants_per_test: variants,
            max_concurrent_requests: None,
            inter_request_delay_ms: None,
            timeout_ms: None,
            enabled_mutators: strategy.enabled_mutators.clone(),
        }
    }

    pub fn with_pacing(mut self, pacing: &promptlab_agent::EndpointPacing) -> Self {
        self.max_concurrent_requests = Some(pacing.effective_concurrency());
        self.inter_request_delay_ms = Some(pacing.inter_request_delay_ms);
        self.timeout_ms = Some(pacing.timeout_ms);
        self
    }
}

fn apply_options_to_budget(ctx: &mut AttackContext, options: Option<&CategoryRunOptions>) {
    if let Some(opts) = options {
        // Never clamp to zero — that yields an empty attack batch and a false "completed".
        ctx.budget.max_payloads = opts.max_payloads.max(1);
        ctx.budget.max_concurrent_requests = opts
            .max_concurrent_requests
            .unwrap_or(DEFAULT_ATTACK_CONCURRENCY)
            .max(1);
        if let Some(delay) = opts.inter_request_delay_ms {
            ctx.budget.inter_request_delay_ms = delay;
        }
        if let Some(timeout) = opts.timeout_ms {
            ctx.budget.timeout_ms = timeout.max(1_000);
        }
        ctx.enabled_mutators = parse_enabled_mutators(opts.enabled_mutators.as_ref());
    } else {
        ctx.budget.max_concurrent_requests = DEFAULT_ATTACK_CONCURRENCY;
    }
}

fn parse_enabled_mutators(raw: Option<&Vec<String>>) -> Option<Vec<MutatorKind>> {
    let Some(ids) = raw else {
        return None;
    };
    Some(ids.iter().filter_map(|id| MutatorKind::parse(id)).collect())
}

async fn apply_category_mutator_plan(
    repos: &Repositories,
    category: AttackCategory,
    ctx: &mut AttackContext,
) {
    use promptlab_storage::MutatorSettingsRepository;

    let Ok(settings) = repos.mutator_settings().get().await else {
        return;
    };
    let Some(ids) = settings.category_mutators.get(category.as_str()) else {
        return;
    };
    ctx.mutator_plan_override = Some(ids.iter().filter_map(|id| MutatorKind::parse(id)).collect());
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
        state.push_phase_with_category(phase, test);
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

#[derive(Clone, Default)]
pub struct CategoryRunResult {
    pub attempts: usize,
    pub successes: u64,
    pub findings: Vec<FindingDto>,
    pub judged: Vec<JudgedAttemptSummary>,
    pub http_successes: u64,
    pub transport_errors: u64,
    pub rate_limited: u64,
    pub server_errors: u64,
    pub avg_latency_ms: u64,
    pub max_latency_ms: u64,
    pub endpoint_unhealthy: bool,
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
            "normalized": attempt.response.normalized.judge_text(),
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

fn judge_severity_from_attack(severity: FindingSeverity) -> JudgeSeverity {
    match severity {
        FindingSeverity::Info => JudgeSeverity::Info,
        FindingSeverity::Low => JudgeSeverity::Low,
        FindingSeverity::Medium => JudgeSeverity::Medium,
        FindingSeverity::High => JudgeSeverity::High,
        FindingSeverity::Critical => JudgeSeverity::Critical,
    }
}

/// When JudgeCoordinator LLM roles are down, keep scanning with attack-side evaluation.
fn fallback_verdict_from_attack_eval(attempt: &PayloadAttempt) -> JudgeVerdict {
    let eval = &attempt.evaluation;
    let vulnerable = eval.success;
    JudgeVerdict {
        probe_id: attempt.payload_id.clone(),
        vulnerable,
        confidence: eval.confidence,
        severity: eval.severity.map(judge_severity_from_attack),
        category: None,
        summary: if eval.summary.trim().is_empty() {
            "Attack evaluation fallback (judge LLM unavailable)".into()
        } else {
            format!("{} [attack-eval fallback]", eval.summary)
        },
        reasoning: "JudgeCoordinator unavailable — used attack evaluation signals".into(),
        evidence: eval.indicators.clone(),
        verdict: if vulnerable {
            "vulnerable".into()
        } else {
            "not_vulnerable".into()
        },
        mode: promptlab_judge::JudgeMode::LocalLlm,
        consensus: promptlab_judge::ConsensusReport {
            agreement_ratio: 1.0,
            participating_evaluators: 0,
            vulnerable_votes: usize::from(vulnerable),
            dissent: false,
            method: "attack_evaluation_fallback".into(),
        },
        evaluator_results: Vec::new(),
        judged_at: OffsetDateTime::now_utc(),
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
    model_manager: Arc<AsyncMutex<promptlab_models::LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    repos: &Repositories,
) -> CommandResult<promptlab_judge::JudgeEngine> {
    let inference = inference_manager.lock().await;
    let manager = model_manager.lock().await;
    let mut runtime_mgr = runtime_manager.lock().await;
    let mut judge = build_judge_engine_from_gateway(
        data_dir,
        &inference,
        &manager,
        model_provider,
        &mut runtime_mgr,
    )
    .await?;
    drop(runtime_mgr);
    drop(manager);
    drop(inference);

    let stored = repos
        .judge_role_weights()
        .get()
        .await
        .map_err(CommandError::from)?;
    judge.set_role_weights(promptlab_judge::RoleWeights {
        judge: stored.judge as f32,
        classifier: stored.classifier as f32,
        attacker: stored.attacker as f32,
        default_llm: stored.default_llm as f32,
    });
    Ok(judge)
}

struct CategoryJudgeAccum {
    successes: u64,
    created_findings: Vec<FindingDto>,
    judged: Vec<JudgedAttemptSummary>,
    http_successes: u64,
    transport_errors: u64,
    rate_limited: u64,
    server_errors: u64,
    latency_total_ms: u64,
    max_latency_ms: u64,
    attempt_count: u64,
}

impl CategoryJudgeAccum {
    fn new() -> Self {
        Self {
            successes: 0,
            created_findings: Vec::new(),
            judged: Vec::new(),
            http_successes: 0,
            transport_errors: 0,
            rate_limited: 0,
            server_errors: 0,
            latency_total_ms: 0,
            max_latency_ms: 0,
            attempt_count: 0,
        }
    }

    fn ingest_attempts(&mut self, attempts: &[PayloadAttempt]) {
        for attempt in attempts {
            self.attempt_count = self.attempt_count.saturating_add(1);
            let status = attempt.response.status;
            let latency = attempt.response.duration_ms;
            self.latency_total_ms = self.latency_total_ms.saturating_add(latency);
            self.max_latency_ms = self.max_latency_ms.max(latency);
            if status == 0 {
                self.transport_errors = self.transport_errors.saturating_add(1);
            } else if status == 429 || status == 503 {
                self.rate_limited = self.rate_limited.saturating_add(1);
            } else if status >= 500 {
                self.server_errors = self.server_errors.saturating_add(1);
            } else if (200..400).contains(&status) {
                self.http_successes = self.http_successes.saturating_add(1);
            }
        }
    }

    fn avg_latency_ms(&self) -> u64 {
        if self.attempt_count == 0 {
            0
        } else {
            self.latency_total_ms / self.attempt_count
        }
    }

    fn endpoint_unhealthy(&self) -> bool {
        if self.rate_limited > 0 {
            return true;
        }
        if self.attempt_count == 0 {
            return false;
        }
        let hard_failures = self
            .transport_errors
            .saturating_add(self.server_errors);
        if self.http_successes == 0 {
            return hard_failures > 0
                || self.avg_latency_ms() >= 10_000
                || self.max_latency_ms >= 20_000;
        }
        // Partial success: unhealthy only when hard failures dominate.
        hard_failures > 0 && hard_failures >= self.http_successes
    }
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
    judge: &'a promptlab_judge::JudgeEngine,
    plugin_manager: &'a Arc<AsyncMutex<promptlab_plugin_host::PluginManager>>,
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

    let judge_request = JudgeRequest::from_normalized(
        attempt.payload_id.clone(),
        env.category_name,
        attempt.mutated_content.clone(),
        normalized,
    );
    let mut verdict: JudgeVerdict = match JudgeCoordinatorAgent::run(&judge_request, env.judge).await
    {
        Ok(out) => out.verdict,
        Err(err) => {
            warn!(
                probe_id = %attempt.payload_id,
                error = %err,
                "JudgeCoordinator failed; falling back to attack evaluation"
            );
            if let Some(emitter) = env.progress {
                emitter.info(format!(
                    "Judge LLM unavailable for {} — attack-eval fallback",
                    attempt.payload_name
                ));
            }
            fallback_verdict_from_attack_eval(&attempt)
        }
    };

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
                    if verdict.evidence.is_empty() {
                        &eval.indicators
                    } else {
                        &verdict.evidence
                    },
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
                .payload(&truncate_console_payload(&attempt.mutated_content, 500))
                .response(&response_console_excerpt(&attempt, 500)),
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

fn response_console_excerpt(attempt: &PayloadAttempt, max_bytes: usize) -> String {
    truncate_console_payload(&attempt.response.normalized.judge_text(), max_bytes)
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
            .latency(attempt.response.duration_ms)
            .response(&response_console_excerpt(attempt, 500)),
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

    let take_attempt =
        |ordered: &mut BTreeMap<usize, PayloadAttempt>,
         next_seq: &mut usize,
         attempts: &mut Vec<PayloadAttempt>,
         emitter: Option<&ScanProgressEmitter>,
         endpoint_url: &str,
         progress_state: Option<&Arc<Mutex<ScanProgress>>>| {
            while let Some(attempt) = ordered.remove(next_seq) {
                if let Some(emitter) = emitter {
                    emit_attack_attempt(emitter, endpoint_url, *next_seq, &attempt);
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
                *next_seq += 1;
            }
        };

    let mut execution_fallback: Option<Vec<PayloadAttempt>> = None;

    loop {
        // Prefer draining streamed attempts over observing completion — otherwise a
        // successful 1-probe run can finish the future before recv is polled and
        // look like an empty batch (false INTERNAL failure + useless recoveries).
        tokio::select! {
            biased;
            item = rx.recv() => {
                match item {
                    Some((seq, attempt)) => {
                        ordered.insert(seq, attempt);
                        take_attempt(
                            &mut ordered,
                            &mut next_seq,
                            &mut attempts,
                            emitter,
                            endpoint_url,
                            progress_state,
                        );
                    }
                    None => break,
                }
            }
            result = exec.as_mut() => {
                match result {
                    Ok(execution) => {
                        while let Ok((seq, attempt)) = rx.try_recv() {
                            ordered.insert(seq, attempt);
                        }
                        if attempts.is_empty() && ordered.is_empty() && !execution.attempts.is_empty()
                        {
                            execution_fallback = Some(execution.attempts);
                        }
                        break;
                    }
                    Err(err) => {
                        // Soft-fail safety net: keep already-streamed probes instead of
                        // discarding partial success and forcing a full recover re-attack.
                        if attempts.is_empty() && ordered.is_empty() {
                            return Err(CommandError::from(
                                promptlab_core::PromptLabError::internal(err.to_string()),
                            ));
                        }
                        if let Some(emitter) = emitter {
                            emitter.warn(format!(
                                "Attack pool ended with error after {} successful probe(s): {err}",
                                attempts.len() + ordered.len()
                            ));
                        }
                        break;
                    }
                }
            }
        }
    }

    while let Ok((seq, attempt)) = rx.try_recv() {
        ordered.insert(seq, attempt);
    }

    take_attempt(
        &mut ordered,
        &mut next_seq,
        &mut attempts,
        emitter,
        endpoint_url,
        progress_state,
    );

    if attempts.is_empty() {
        if let Some(fallback) = execution_fallback {
            for (seq, attempt) in fallback.into_iter().enumerate() {
                ordered.insert(seq, attempt);
            }
            take_attempt(
                &mut ordered,
                &mut next_seq,
                &mut attempts,
                emitter,
                endpoint_url,
                progress_state,
            );
        }
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
    let mut accum = CategoryJudgeAccum::new();

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
            let cancelled = env
                .job_controls
                .map(|ctrl| ctrl.cancel.load(Ordering::Relaxed))
                .unwrap_or(false);
            if cancelled {
                return Ok(category_result_from_accum(&accum));
            }
            return Err(CommandError::from(promptlab_core::PromptLabError::internal(
                format!(
                    "no attack attempts collected for {} (empty payload batch or executor skipped all)",
                    category_label
                ),
            )));
        }

        // Ingest once per category run (checkpoint resume must not double-count).
        if accum.attempt_count == 0 {
            accum.ingest_attempts(&attempts);
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

            // Soft-failed transport probes are already counted in health stats — skip judge.
            if attempts[seq].response.status == 0 {
                if let Some(progress_state) = env.progress_state {
                    bump_scan_progress(progress_state, 1);
                }
                continue;
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
        // Prefer HTTP attempt count so an empty judge list cannot look like "no work".
        attempts: accum
            .attempt_count
            .max(accum.judged.len() as u64) as usize,
        successes: accum.successes,
        findings: accum.created_findings.clone(),
        judged: accum.judged.clone(),
        http_successes: accum.http_successes,
        transport_errors: accum.transport_errors,
        rate_limited: accum.rate_limited,
        server_errors: accum.server_errors,
        avg_latency_ms: accum.avg_latency_ms(),
        max_latency_ms: accum.max_latency_ms,
        endpoint_unhealthy: accum.endpoint_unhealthy(),
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
    model_manager: Arc<AsyncMutex<promptlab_models::LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    plugin_manager: Arc<AsyncMutex<promptlab_plugin_host::PluginManager>>,
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
    apply_options_to_budget(&mut ctx, options);
    apply_category_mutator_plan(repos, category, &mut ctx).await;
    let variants = options
        .map(|opts| opts.variants_per_test.max(1))
        .unwrap_or(1);
    let executor = attack_executor_with_variants(runtime.transport, variants);

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
        repos,
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
    model_manager: Arc<AsyncMutex<promptlab_models::LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    plugin_manager: Arc<AsyncMutex<promptlab_plugin_host::PluginManager>>,
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
    apply_options_to_budget(&mut ctx, options);
    apply_category_mutator_plan(repos, category, &mut ctx).await;
    let variants = options
        .map(|opts| opts.variants_per_test.max(1))
        .unwrap_or(1);
    let executor = attack_executor_with_variants(runtime.transport, variants);

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
        repos,
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
