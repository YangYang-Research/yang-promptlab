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
    AttackExecutor, FindingSeverity, PayloadAttempt,
};
use aisec_endpoint_metadata::body_template_from_metadata;
use crate::dto::metadata_from_endpoint;
use aisec_target_profile::TargetProfile;
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

use std::sync::Arc;
use std::collections::HashMap;
use std::sync::Mutex;

use tauri::async_runtime::Mutex as AsyncMutex;

use crate::dto::{AttackRunDto, FindingDto, ScanDto};
use crate::error::{CommandError, CommandResult};
use crate::events::{ScanProgressEmitter, ScanProgressLevel};
use crate::inference_host::build_judge_engine_from_gateway;
use crate::session_auth::{attack_executor, attack_executor_with_variants, build_attack_runtime, AttackRuntime};
use crate::jobs::ScanProgress;
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

fn increment_scan_progress(progress: Option<&Arc<Mutex<ScanProgress>>>, units: u64) {
    let Some(progress) = progress else {
        return;
    };
    if let Ok(mut state) = progress.lock() {
        state.completed = state.completed.saturating_add(units).min(state.total);
    }
}

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

    let result = executor
        .execute_category(category, &ctx)
        .await
        .map_err(|err| CommandError::from(aisec_core::AisecError::internal(err.to_string())))?;

    update_scan_phase(
        progress_state,
        "judge",
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
    let category_name = category.as_str();
    let mut successes = 0u64;
    let mut created_findings: Vec<FindingDto> = Vec::new();
    let mut judged: Vec<JudgedAttemptSummary> = Vec::new();

    for (index, attempt) in result.attempts.iter().enumerate() {
        let eval = &attempt.evaluation;
        let normalized = &attempt.response.normalized;

        if let Some(emitter) = progress {
            emitter.detailed(
                ScanProgressLevel::Info,
                emitter
                    .event(ScanProgressLevel::Info, format!("Payload #{} {}", index + 1, attempt.payload_name))
                    .payload(&attempt.payload_name)
                    .endpoint(&endpoint.url),
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
                    .endpoint(&endpoint.url)
                    .status_code(attempt.response.status)
                    .latency(attempt.response.duration_ms),
            );
        }

        let mut verdict: JudgeVerdict = judge
            .judge_normalized(
                attempt.payload_id.clone(),
                category_name,
                attempt.mutated_content.clone(),
                normalized,
            )
            .await
            .map_err(|err| CommandError::from(aisec_core::AisecError::internal(err.to_string())))?;

        {
            let mut plugins = plugin_manager.lock().await;
            if let Ok(signals) = evaluate_with_judge_plugins(
                &mut plugins,
                &attempt.response.body,
                category_name,
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

        repos
            .attack_results()
            .create(CreateAttackResult {
                scan_id: scan_id.to_string(),
                payload_id: None,
                target_id: target_id.clone(),
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
            successes += 1;
            let severity = verdict
                .severity
                .map(judge_severity_str)
                .or_else(|| eval.severity.map(severity_str))
                .unwrap_or("medium");
            let finding = repos
                .findings()
                .create(CreateFinding {
                    scan_id: scan_id.to_string(),
                    project_id: project_id.to_string(),
                    target_id: target_id.clone(),
                    title: format!("{}: {}", category.display_name(), attempt.payload_name),
                    severity: severity.to_string(),
                    category: Some(category_name.into()),
                    description: Some(verdict.summary.clone()),
                    evidence_json: Some(finding_evidence_json(
                        &endpoint.url,
                        endpoint.method.as_deref(),
                        metadata_from_endpoint(endpoint)
                            .map(|meta| body_template_from_metadata(&meta))
                            .as_deref(),
                        attempt,
                        &verdict.summary,
                        verdict.confidence,
                        &eval.indicators,
                        &judge_json,
                        None,
                    )),
                    status: None,
                })
                .await
                .map_err(CommandError::from)?;
            let finding_dto = FindingDto::from(finding);
            created_findings.push(finding_dto.clone());
            if let Some(emitter) = progress {
                emitter.detailed(
                    ScanProgressLevel::Info,
                    emitter
                        .event(ScanProgressLevel::Info, "Saved finding")
                        .endpoint(&endpoint.url)
                        .finding_id(&finding_dto.id),
                );
            }
        }

        judged.push(JudgedAttemptSummary {
            payload_id: attempt.payload_id.clone(),
            payload_name: attempt.payload_name.clone(),
            vulnerable: verdict.vulnerable,
            confidence: verdict.confidence,
            summary: verdict.summary.clone(),
        });
        increment_scan_progress(progress_state, 1);

        if let Some(emitter) = progress {
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
                    .endpoint(&endpoint.url)
                    .payload(&attempt.payload_name),
            );
        }
    }

    Ok(CategoryRunResult {
        attempts: result.attempts.len(),
        successes,
        findings: created_findings,
        judged,
    })
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
) -> CommandResult<CategoryRunResult> {
    let url = profile.full_url();
    let mut target = AttackTarget::llm_api(url.clone());
    target.method = Some(profile.method.as_str().into());
    target.body_template = Some(profile.request_template.clone());
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

    let result = executor
        .execute_category(category, &ctx)
        .await
        .map_err(|err| CommandError::from(aisec_core::AisecError::internal(err.to_string())))?;

    update_scan_phase(
        progress_state,
        "judge",
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
    let category_name = category.as_str();
    let mut successes = 0u64;
    let mut created_findings: Vec<FindingDto> = Vec::new();
    let mut judged: Vec<JudgedAttemptSummary> = Vec::new();

    for (index, attempt) in result.attempts.iter().enumerate() {
        let eval = &attempt.evaluation;
        let normalized = &attempt.response.normalized;

        if let Some(emitter) = progress {
            emitter.detailed(
                ScanProgressLevel::Info,
                emitter
                    .event(ScanProgressLevel::Info, format!("Payload #{} {}", index + 1, attempt.payload_name))
                    .payload(&attempt.payload_name)
                    .endpoint(&url),
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
                    .endpoint(&url)
                    .status_code(attempt.response.status)
                    .latency(attempt.response.duration_ms),
            );
        }

        let mut verdict: JudgeVerdict = judge
            .judge_normalized(
                attempt.payload_id.clone(),
                category_name,
                attempt.mutated_content.clone(),
                normalized,
            )
            .await
            .map_err(|err| CommandError::from(aisec_core::AisecError::internal(err.to_string())))?;

        {
            let mut plugins = plugin_manager.lock().await;
            if let Ok(signals) = evaluate_with_judge_plugins(
                &mut plugins,
                &attempt.response.body,
                category_name,
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

        repos
            .attack_results()
            .create(CreateAttackResult {
                scan_id: scan_id.to_string(),
                payload_id: None,
                target_id: Some(target_id.to_string()),
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
            successes += 1;
            let severity = verdict
                .severity
                .map(judge_severity_str)
                .or_else(|| eval.severity.map(severity_str))
                .unwrap_or("medium");
            let finding = repos
                .findings()
                .create(CreateFinding {
                    scan_id: scan_id.to_string(),
                    project_id: project_id.to_string(),
                    target_id: Some(target_id.to_string()),
                    title: format!("{}: {}", category.display_name(), attempt.payload_name),
                    severity: severity.to_string(),
                    category: Some(category_name.into()),
                    description: Some(verdict.summary.clone()),
                    evidence_json: Some(finding_evidence_json(
                        &url,
                        Some(profile.method.as_str()),
                        Some(profile.request_template.as_str()),
                        attempt,
                        &verdict.summary,
                        verdict.confidence,
                        &eval.indicators,
                        &judge_json,
                        Some(profile.provider.as_str()),
                    )),
                    status: None,
                })
                .await
                .map_err(CommandError::from)?;
            let finding_dto = FindingDto::from(finding);
            created_findings.push(finding_dto.clone());
            if let Some(emitter) = progress {
                emitter.detailed(
                    ScanProgressLevel::Info,
                    emitter
                        .event(ScanProgressLevel::Info, "Saved finding")
                        .endpoint(&url)
                        .finding_id(&finding_dto.id),
                );
            }
        }

        judged.push(JudgedAttemptSummary {
            payload_id: attempt.payload_id.clone(),
            payload_name: attempt.payload_name.clone(),
            vulnerable: verdict.vulnerable,
            confidence: verdict.confidence,
            summary: verdict.summary.clone(),
        });
        increment_scan_progress(progress_state, 1);

        if let Some(emitter) = progress {
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
                    .endpoint(&url)
                    .payload(&attempt.payload_name),
            );
        }
    }

    Ok(CategoryRunResult {
        attempts: result.attempts.len(),
        successes,
        findings: created_findings,
        judged,
    })
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
