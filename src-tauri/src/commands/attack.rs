//! Attack execution commands for scan jobs and category runs.

use promptlab_attack::{
    apply_descriptor_auth, AttackCategory, AttackContext, AttackPayload, AttackTarget,
    AttackExecutor, FindingSeverity, MutatorKind, PayloadAttempt, DEFAULT_ATTACK_CONCURRENCY,
};
use promptlab_endpoint_metadata::body_template_from_metadata;
use crate::dto::metadata_from_endpoint;
use promptlab_target_profile::{TargetProfile, TargetProvider};
use promptlab_auth::{resolve_descriptor_for_runtime, resolve_descriptor_for_wizard, SecretStore};
use promptlab_agent::JudgeCoordinatorAgent;
use promptlab_judge::{JudgeRequest, JudgeVerdict, Severity as JudgeSeverity};
use promptlab_planner::PlannerLlm;
use promptlab_plugin_host::evaluate_with_judge_plugins;
use promptlab_inference::InferenceRuntimeManager;
use promptlab_runtime::{RuntimeManager, SharedModelProvider};
use promptlab_storage::{
    AttackResultRepository, CreateAttackResult, CreateFinding, Endpoint, FindingRepository,
    JudgeRoleWeightsRepository, Repositories, ScanRepository, TargetRepository, UpdateFinding,
    UpdateScan,
};
use time::OffsetDateTime;
use tracing::{info, warn};

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
use crate::session_auth::{attack_executor_with_variants, AttackRuntime};
use crate::jobs::{bump_scan_progress, ScanBatchCheckpoint, ScanJobControls, ScanProgress};
use crate::scan_playbook::persist_scan_playbook_state;

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
        state.apply_phase(phase, test, attempt, retry);
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
        TargetProvider::GenericHttp => "rest_api",
        TargetProvider::Mcp => "mcp_server",
        TargetProvider::Dify => "dify",
        TargetProvider::AnthropicClaude => "anthropic_compatible",
        TargetProvider::GoogleGemini => "gemini",
        TargetProvider::AwsBedrock => "bedrock",
        TargetProvider::GenericWebSocket => "websocket",
        TargetProvider::OpenAiCompatible
        | TargetProvider::OpenRouter
        | TargetProvider::AzureOpenAi
        | TargetProvider::GitHubCopilot
        | TargetProvider::OpenWebUi
        | TargetProvider::Langflow => "openai_compatible",
    }
}

fn finding_evidence_json(
    url: &str,
    method: Option<&str>,
    body_template: Option<&str>,
    request_headers: &std::collections::HashMap<String, String>,
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
            "headers": request_headers,
            "body": attempt.mutated_content,
            "body_template": body_template,
        },
        "response": {
            "status": attempt.response.status,
            "headers": attempt.response.headers,
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

fn technique_key(payload_id: &str) -> String {
    payload_id
        .split(':')
        .next()
        .unwrap_or(payload_id)
        .trim()
        .to_string()
}

const CLUSTER_MIN_CONFIDENCE: f32 = 0.75;

fn verdict_has_canary_echoed(verdict: &JudgeVerdict) -> bool {
    verdict.evidence.iter().any(|e| {
        e == "canary_echoed" || e.starts_with("canary_echoed:")
    })
}

fn verdict_has_payload_echo(verdict: &JudgeVerdict) -> bool {
    verdict.evidence.iter().any(|e| e == "canary_payload_echo")
}

/// Weak / payload-echo votes stay on the attack result; they do not become findings.
/// A true `canary_echoed` vote still qualifies even if another worker also tagged payload_echo.
fn finding_qualifies_for_cluster(verdict: &JudgeVerdict) -> bool {
    if !verdict.vulnerable {
        return false;
    }
    if verdict_has_payload_echo(verdict) && !verdict_has_canary_echoed(verdict) {
        return false;
    }
    verdict.confidence >= CLUSTER_MIN_CONFIDENCE || verdict_has_canary_echoed(verdict)
}

fn clustered_finding_description(
    max_confidence: f32,
    variant_count: usize,
    canary_hits: usize,
) -> String {
    let pct = (max_confidence * 100.0).round().clamp(0.0, 100.0) as u32;
    let mut out = format!("Vulnerability detected with {pct}% confidence");
    if variant_count > 1 {
        out.push_str(&format!("; {variant_count} variant(s)"));
    }
    if canary_hits > 0 {
        out.push_str(&format!("; {canary_hits} canary hit(s)"));
    }
    out
}

fn cluster_variant_stats(evidence: &serde_json::Value) -> (f32, usize, usize, String) {
    let variants = evidence
        .get("variants")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let variant_count = variants.len().max(1);
    let mut max_conf = evidence
        .get("confidence")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0) as f32;
    let mut canary_hits = 0usize;
    let mut max_severity = "info".to_string();
    for variant in &variants {
        if let Some(conf) = variant.get("confidence").and_then(|v| v.as_f64()) {
            max_conf = max_conf.max(conf as f32);
        }
        if variant
            .get("canary_echoed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            canary_hits += 1;
        }
        if let Some(sev) = variant.get("severity").and_then(|v| v.as_str()) {
            if severity_rank(sev) > severity_rank(&max_severity) {
                max_severity = sev.to_string();
            }
        }
    }
    (max_conf, variant_count, canary_hits, max_severity)
}

fn evidence_technique_key(evidence_json: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(evidence_json).ok()?;
    if let Some(key) = value.get("technique_id").and_then(|id| id.as_str()) {
        let key = key.trim();
        if !key.is_empty() {
            return Some(key.to_string());
        }
    }
    value
        .get("payload_id")
        .and_then(|id| id.as_str())
        .map(technique_key)
}

fn severity_rank(severity: &str) -> u8 {
    match severity.trim().to_ascii_lowercase().as_str() {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

fn variant_evidence_entry(
    attempt: &PayloadAttempt,
    verdict: &JudgeVerdict,
    severity: &str,
) -> serde_json::Value {
    serde_json::json!({
        "payload_id": attempt.payload_id,
        "confidence": verdict.confidence,
        "summary": verdict.summary,
        "status": attempt.response.status,
        "duration_ms": attempt.response.duration_ms,
        "severity": severity,
        "canary_echoed": verdict_has_canary_echoed(verdict),
    })
}

async fn persist_clustered_finding(
    env: &CategoryJudgeEnv<'_>,
    attempt: &PayloadAttempt,
    verdict: &JudgeVerdict,
    eval_indicators: &[String],
    judge_json: &serde_json::Value,
    severity: &str,
) -> CommandResult<(FindingDto, bool)> {
    let technique = technique_key(&attempt.payload_id);
    let indicators = if verdict.evidence.is_empty() {
        eval_indicators
    } else {
        &verdict.evidence
    };
    let mut evidence = finding_evidence_json(
        env.endpoint_url,
        env.method,
        env.body_template,
        env.request_headers,
        attempt,
        &verdict.summary,
        verdict.confidence,
        indicators,
        judge_json,
        env.provider,
    );
    if let Some(obj) = evidence.as_object_mut() {
        obj.insert("technique_id".into(), serde_json::json!(technique));
        obj.insert(
            "variants".into(),
            serde_json::json!([variant_evidence_entry(attempt, verdict, severity)]),
        );
    }

    let existing = env
        .repos
        .findings()
        .list_by_scan(env.scan_id)
        .await
        .map_err(CommandError::from)?;
    if let Some(found) = existing.iter().find(|finding| {
        finding
            .evidence_json
            .as_deref()
            .and_then(evidence_technique_key)
            .as_deref()
            == Some(technique.as_str())
    }) {
        let mut merged = found
            .evidence_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
            .unwrap_or_else(|| serde_json::json!({}));
        if let Some(obj) = merged.as_object_mut() {
            let variants = obj
                .entry("variants")
                .or_insert_with(|| serde_json::json!([]));
            if let Some(list) = variants.as_array_mut() {
                list.push(variant_evidence_entry(attempt, verdict, severity));
            }
            obj.insert("technique_id".into(), serde_json::json!(technique));
        }
        let (max_conf, variant_count, canary_hits, variant_severity) =
            cluster_variant_stats(&merged);
        if let Some(obj) = merged.as_object_mut() {
            obj.insert("confidence".into(), serde_json::json!(max_conf));
            obj.insert(
                "explanation".into(),
                serde_json::json!(clustered_finding_description(
                    max_conf,
                    variant_count,
                    canary_hits
                )),
            );
        }
        let next_severity = if severity_rank(severity) > severity_rank(&found.severity) {
            severity.to_string()
        } else if severity_rank(&variant_severity) > severity_rank(&found.severity) {
            variant_severity
        } else {
            found.severity.clone()
        };
        let finding = env
            .repos
            .findings()
            .update(
                &found.id,
                UpdateFinding {
                    severity: Some(next_severity),
                    description: Some(clustered_finding_description(
                        max_conf,
                        variant_count,
                        canary_hits,
                    )),
                    evidence_json: Some(merged),
                    ..Default::default()
                },
            )
            .await
            .map_err(CommandError::from)?;
        return Ok((FindingDto::from(finding), false));
    }

    let (max_conf, variant_count, canary_hits, _) = cluster_variant_stats(&evidence);
    if let Some(obj) = evidence.as_object_mut() {
        obj.insert("confidence".into(), serde_json::json!(max_conf));
        obj.insert(
            "explanation".into(),
            serde_json::json!(clustered_finding_description(
                max_conf,
                variant_count,
                canary_hits
            )),
        );
    }
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
            description: Some(clustered_finding_description(
                max_conf,
                variant_count,
                canary_hits,
            )),
            evidence_json: Some(evidence),
            status: None,
        })
        .await
        .map_err(CommandError::from)?;
    Ok((FindingDto::from(finding), true))
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
pub(crate) async fn build_judge_for_category(
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

pub(crate) fn judge_coordinator_llm(
    data_dir: &std::path::Path,
    inference_manager: Arc<AsyncMutex<InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<promptlab_models::LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
) -> Arc<dyn PlannerLlm> {
    Arc::new(
        crate::inference_host::HostYazgReactLlm::new(
            data_dir.to_path_buf(),
            inference_manager,
            model_manager,
            model_provider,
            runtime_manager,
        )
        .with_agent_id("judge_coordinator"),
    )
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
    request_headers: &'a std::collections::HashMap<String, String>,
    provider: Option<&'a str>,
    judge: Arc<promptlab_judge::JudgeEngine>,
    orchestrator: Arc<dyn PlannerLlm>,
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

    let judge_request = {
        let mut req = JudgeRequest::from_normalized(
            attempt.payload_id.clone(),
            env.category_name,
            attempt.mutated_content.clone(),
            normalized,
        );
        if let Some(canary) = promptlab_core::find_canary_in(&attempt.mutated_content) {
            if let Some(obj) = req.context.as_object_mut() {
                obj.insert("expected_canary".into(), serde_json::Value::String(canary));
            }
        }
        req
    };
    let mut verdict: JudgeVerdict = match JudgeCoordinatorAgent::run_with_orchestrator(
        &judge_request,
        Arc::clone(&env.judge),
        Arc::clone(&env.orchestrator),
    )
    .await
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

    if verdict.vulnerable && finding_qualifies_for_cluster(&verdict) {
        accum.successes += 1;
        let severity = verdict
            .severity
            .map(judge_severity_str)
            .or_else(|| eval.severity.map(severity_str))
            .unwrap_or("medium");
        let (finding_dto, created) = persist_clustered_finding(
            env,
            &attempt,
            &verdict,
            &eval.indicators,
            &judge_json,
            severity,
        )
        .await?;
        if created {
            accum.created_findings.push(finding_dto.clone());
            if let Some(progress_state) = env.progress_state {
                if let Ok(mut state) = progress_state.lock() {
                    state.note_finding();
                }
            }
        }
        if let Some(emitter) = env.progress {
            let message = if created {
                "Saved finding"
            } else {
                "Updated finding (variant clustered)"
            };
            emitter.detailed(
                ScanProgressLevel::Info,
                emitter
                    .event(ScanProgressLevel::Info, message)
                    .endpoint(env.endpoint_url)
                    .finding_id(&finding_dto.id),
            );
        }
    } else if verdict.vulnerable {
        accum.successes += 1;
        if let Some(emitter) = env.progress {
            emitter.info(format!(
                "Skipped finding for {} (confidence {:.0}%, no canary_echoed)",
                attempt.payload_name,
                verdict.confidence * 100.0
            ));
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
                        state.note_attack();
                        state.note_testcase(&attempt.payload_id);
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

async fn persist_interrupt_checkpoint(
    controls: &ScanJobControls,
    checkpoint: ScanBatchCheckpoint,
    repos: &Repositories,
    scan_id: &str,
    progress_state: Option<&Arc<Mutex<ScanProgress>>>,
) {
    let checkpoint_for_db = checkpoint.clone();
    *controls.batch_checkpoint.lock().unwrap() = Some(checkpoint);

    let progress_snapshot = progress_state.and_then(|progress_state| {
        progress_state.lock().ok().map(|progress| progress.clone())
    });

    if let Some(progress) = progress_snapshot.as_ref() {
        let _ = persist_scan_playbook_state(
            repos,
            scan_id,
            Some(progress),
            Some(Some(&checkpoint_for_db)),
        )
        .await;
    } else {
        let _ = persist_scan_playbook_state(repos, scan_id, None, Some(Some(&checkpoint_for_db)))
            .await;
    }
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
                let matches_category = match guard.as_ref() {
                    Some(ScanBatchCheckpoint::PendingJudge { category, .. })
                    | Some(ScanBatchCheckpoint::JudgingPartial { category, .. }) => {
                        category == &category_label
                            || category.eq_ignore_ascii_case(&category_label)
                    }
                    None => false,
                };
                if matches_category {
                    guard.take()
                } else {
                    None
                }
            };
            if let Some(checkpoint) = restored_checkpoint {
                if let Some(emitter) = env.progress {
                    let (resume_len, resume_index) = match &checkpoint {
                        ScanBatchCheckpoint::PendingJudge { attempts, .. } => (attempts.len(), 0usize),
                        ScanBatchCheckpoint::JudgingPartial {
                            attempts,
                            next_judge_index,
                            ..
                        } => (attempts.len(), *next_judge_index),
                    };
                    emitter.info(format!(
                        "Resuming {category_label} from checkpoint — judging from {resume_index}/{resume_len} (skipping attack collect)"
                    ));
                }
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
                if !collected.is_empty() {
                    persist_interrupt_checkpoint(
                        ctrl,
                        ScanBatchCheckpoint::PendingJudge {
                            category: category_label.to_string(),
                            attempts: collected.clone(),
                        },
                        env.repos,
                        env.scan_id,
                        env.progress_state,
                    )
                    .await;
                }
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
                    persist_interrupt_checkpoint(
                        ctrl,
                        ScanBatchCheckpoint::JudgingPartial {
                            category: category_label.to_string(),
                            attempts: attempts.clone(),
                            next_judge_index: seq,
                        },
                        env.repos,
                        env.scan_id,
                        env.progress_state,
                    )
                    .await;
                    return Ok(category_result_from_accum(&accum));
                }
            }

            // Soft-failed transport probes are already counted in health stats — skip judge.
            if attempts[seq].response.status == 0 {
                if let Some(progress_state) = env.progress_state {
                    bump_scan_progress(progress_state, 1);
                }
                if let Some(ctrl) = env.job_controls {
                    persist_interrupt_checkpoint(
                        ctrl,
                        ScanBatchCheckpoint::JudgingPartial {
                            category: category_label.to_string(),
                            attempts: attempts.clone(),
                            next_judge_index: seq + 1,
                        },
                        env.repos,
                        env.scan_id,
                        env.progress_state,
                    )
                    .await;
                }
                continue;
            }

            judge_single_attempt(env, seq, attempts[seq].clone(), &mut accum).await?;

            if let Some(ctrl) = env.job_controls {
                persist_interrupt_checkpoint(
                    ctrl,
                    ScanBatchCheckpoint::JudgingPartial {
                        category: category_label.to_string(),
                        attempts: attempts.clone(),
                        next_judge_index: seq + 1,
                    },
                    env.repos,
                    env.scan_id,
                    env.progress_state,
                )
                .await;
            }

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

    let orchestrator = judge_coordinator_llm(
        data_dir,
        Arc::clone(&inference_manager),
        Arc::clone(&model_manager),
        model_provider.clone(),
        Arc::clone(&runtime_manager),
    );
    let judge = Arc::new(
        build_judge_for_category(
            data_dir,
            inference_manager,
            model_manager,
            model_provider.clone(),
            runtime_manager,
            repos,
        )
        .await?,
    );
    let endpoint_metadata = metadata_from_endpoint(endpoint);
    let body_template = endpoint_metadata
        .as_ref()
        .map(|meta| body_template_from_metadata(meta));
    let request_headers = ctx.target.headers.clone();
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
        request_headers: &request_headers,
        provider: None,
        judge,
        orchestrator,
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

    let orchestrator = judge_coordinator_llm(
        data_dir,
        Arc::clone(&inference_manager),
        Arc::clone(&model_manager),
        model_provider.clone(),
        Arc::clone(&runtime_manager),
    );
    let judge = Arc::new(
        build_judge_for_category(
            data_dir,
            inference_manager,
            model_manager,
            model_provider.clone(),
            runtime_manager,
            repos,
        )
        .await?,
    );
    let request_headers = ctx.target.headers.clone();
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
        request_headers: &request_headers,
        provider: Some(profile.provider.as_str()),
        judge,
        orchestrator,
        plugin_manager: &plugin_manager,
        progress,
        progress_state,
        job_controls,
    };

    execute_category_then_judge(executor, category, ctx, &judge_env).await
}

#[cfg(test)]
mod tests {
    use super::{
        cluster_variant_stats, clustered_finding_description, finding_qualifies_for_cluster,
        truncate_console_payload,
    };
    use promptlab_judge::{ConsensusReport, JudgeMode, JudgeVerdict};
    use time::OffsetDateTime;

    fn verdict(vulnerable: bool, confidence: f32, evidence: &[&str]) -> JudgeVerdict {
        JudgeVerdict {
            probe_id: "p".into(),
            vulnerable,
            confidence,
            severity: None,
            category: None,
            summary: String::new(),
            reasoning: String::new(),
            evidence: evidence.iter().map(|s| (*s).to_string()).collect(),
            verdict: String::new(),
            mode: JudgeMode::LocalLlm,
            consensus: ConsensusReport {
                agreement_ratio: 1.0,
                participating_evaluators: 1,
                vulnerable_votes: usize::from(vulnerable),
                dissent: false,
                method: "test".into(),
            },
            evaluator_results: Vec::new(),
            judged_at: OffsetDateTime::now_utc(),
        }
    }

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

    #[test]
    fn clustered_description_uses_max_confidence() {
        assert_eq!(
            clustered_finding_description(0.694, 5, 0),
            "Vulnerability detected with 69% confidence; 5 variant(s)"
        );
        assert_eq!(
            clustered_finding_description(1.0, 3, 2),
            "Vulnerability detected with 100% confidence; 3 variant(s); 2 canary hit(s)"
        );
    }

    #[test]
    fn cluster_stats_take_max_confidence_and_canary_hits() {
        let evidence = serde_json::json!({
            "confidence": 0.55,
            "variants": [
                {"confidence": 0.55, "severity": "medium", "canary_echoed": false},
                {"confidence": 0.91, "severity": "high", "canary_echoed": true},
            ]
        });
        let (conf, count, canary, sev) = cluster_variant_stats(&evidence);
        assert!((conf - 0.91).abs() < 0.001);
        assert_eq!(count, 2);
        assert_eq!(canary, 1);
        assert_eq!(sev, "high");
    }

    #[test]
    fn payload_echo_without_echoed_does_not_qualify() {
        assert!(!finding_qualifies_for_cluster(&verdict(
            true,
            1.0,
            &["canary_payload_echo"]
        )));
    }

    #[test]
    fn high_confidence_without_payload_echo_qualifies() {
        assert!(finding_qualifies_for_cluster(&verdict(true, 0.9, &["print"])));
    }

    #[test]
    fn echoed_qualifies_even_with_payload_echo() {
        assert!(finding_qualifies_for_cluster(&verdict(
            true,
            0.4,
            &["canary_echoed", "canary_payload_echo"]
        )));
    }
}
