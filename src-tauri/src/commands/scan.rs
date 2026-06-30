//! Scan lifecycle commands — background attack orchestration.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::collections::HashMap;

use aisec_auth::AuthEngineConfig;
use aisec_attack::{AttackCategory, AttackPayload};
use aisec_models::LocalModelManager;
use aisec_runtime::SharedModelProvider;
use aisec_storage::{
    CreateScan, EndpointRepository, FindingRepository, ProjectRepository, Repositories,
    ScanRepository, TargetRepository, UpdateScan,
};
use tauri::async_runtime::Mutex as AsyncMutex;
use tauri::{AppHandle, State};
use time::OffsetDateTime;
use tracing::{info, instrument, warn};

use crate::commands::attack::{run_category_on_target_profile, run_category_on_endpoint};
use crate::agent_service::{agent_config_from_scan, run_agent_endpoint, ScanAgentHost};
use crate::commands::generator::{
    attack_plan_from_scan, generate_payloads_for_scan_job, generate_payloads_for_scan_job_with_strategy,
    parse_generator_mode_optional, prompt_payloads_map,
};
use aisec_target_profile::PayloadStrategy;
use crate::events::{emit_app_data_changed, ScanProgressEmitter};
use crate::session_auth::{build_attack_runtime_parts, fallback_attack_runtime, seed_url_from_descriptor, AttackRuntime};
use crate::commands::target_profile::parse_target_profile;
use crate::dto::{ScanStartDto, ScanStatusDto};
use crate::error::{CommandError, CommandResult};
use crate::jobs::{ScanJobManager, ScanProgress};
use crate::state::AppState;

fn parse_category(value: &str) -> Option<AttackCategory> {
    AttackCategory::all()
        .iter()
        .copied()
        .find(|cat| cat.as_str() == value)
}

fn is_restartable_scan_status(status: &str) -> bool {
    matches!(status, "failed" | "cancelled" | "stopped" | "completed")
}

fn merge_scan_execution_playbook(
    existing_playbook_json: Option<&str>,
    execution_playbook: &mut serde_json::Value,
    clear_progress: bool,
) -> serde_json::Value {
    let mut playbook = existing_playbook_json
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(wizard) = playbook.get("wizard").cloned() {
        execution_playbook["wizard_snapshot"] = wizard;
    }
    if let Some(obj) = playbook.as_object_mut() {
        if clear_progress {
            obj.remove("progress");
        }
        if let Some(execution) = execution_playbook.as_object() {
            for (key, value) in execution {
                obj.insert(key.clone(), value.clone());
            }
        }
    }
    playbook
}

fn progress_to_dto(scan_id: &str, progress: &ScanProgress) -> ScanStatusDto {
    ScanStatusDto {
        scan_id: scan_id.to_string(),
        status: progress.status.clone(),
        progress_percent: progress.progress_percent(),
        completed: progress.completed,
        total: progress.total,
        findings_count: progress.findings,
        current_endpoint: progress.current_endpoint.clone(),
        current_test: progress.current_test.clone(),
        started_at: progress.started_at.clone(),
        agent_mode: progress.agent_mode,
        current_phase: progress.current_phase.clone(),
        current_attempt: progress.current_attempt,
        current_retry: progress.current_retry,
    }
}

fn progress_from_playbook(playbook_json: Option<&str>) -> Option<ScanProgress> {
    let raw = playbook_json?;
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    serde_json::from_value(value.get("progress")?.clone()).ok()
}

async fn persist_playbook_progress(
    repos: &Repositories,
    scan_id: &str,
    progress: &ScanProgress,
) -> Result<(), aisec_core::AisecError> {
    let scan = repos.scans().get(scan_id).await?;
    let mut playbook = scan
        .playbook_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    if let Some(obj) = playbook.as_object_mut() {
        obj.insert(
            "progress".into(),
            serde_json::to_value(progress).unwrap_or(serde_json::Value::Null),
        );
    }

    repos
        .scans()
        .update(
            scan_id,
            UpdateScan {
                playbook_json: Some(playbook),
                ..Default::default()
            },
        )
        .await?;
    Ok(())
}

async fn wait_if_paused(paused: &AtomicBool, cancel: &AtomicBool) {
    while paused.load(Ordering::Relaxed) {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn run_scan_job(
    app: AppHandle,
    db: aisec_storage::Database,
    jobs: ScanJobManager,
    scan_id: String,
    project_id: String,
    target_id: Option<String>,
    categories: Vec<AttackCategory>,
    disabled_tests: Vec<String>,
    profile: String,
    generator_mode: Option<String>,
    payload_strategy: Option<PayloadStrategy>,
    cancel: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    progress: Arc<Mutex<ScanProgress>>,
    data_dir: std::path::PathBuf,
    auth_config: AuthEngineConfig,
    harness_factory: aisec_harness::HarnessFactory,
    plugin_manager: Arc<AsyncMutex<aisec_plugin_host::PluginManager>>,
    inference_manager: Arc<AsyncMutex<aisec_inference::InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<aisec_runtime::RuntimeManager>>,
) {
    let repos = db.repositories();
    let progress_emitter = ScanProgressEmitter::new(app.clone(), scan_id.clone());
    progress_emitter.info("Loading attack plan...");
    let default_runtime = fallback_attack_runtime();

    let target_profile = if let Some(tid) = &target_id {
        repos
            .targets()
            .get(tid)
            .await
            .ok()
            .and_then(|t| parse_target_profile(&t.profile_json).ok())
    } else {
        None
    };

    let attack_runtime: AttackRuntime = if let Some(tid) = &target_id {
        if let Ok(target) = repos.targets().get(tid).await {
            let probe_url = target_profile
                .as_ref()
                .map(|p| p.full_url())
                .or_else(|| seed_url_from_descriptor(&target.descriptor_json))
                .unwrap_or_else(|| "https://localhost".into());
            build_attack_runtime_parts(
                db.clone(),
                &data_dir,
                auth_config.clone(),
                &harness_factory,
                plugin_manager.clone(),
                &target.descriptor_json,
                &probe_url,
            )
            .await
            .unwrap_or(default_runtime)
        } else {
            default_runtime
        }
    } else {
        default_runtime
    };

    let generated_payloads: Option<HashMap<AttackCategory, Vec<AttackPayload>>> =
        if let Some(ref strategy) = payload_strategy {
            let plan = attack_plan_from_scan(profile.clone(), categories.clone(), disabled_tests.clone());
            match generate_payloads_for_scan_job_with_strategy(
                &data_dir,
                Arc::clone(&inference_manager),
                Arc::clone(&model_manager),
                model_provider.clone(),
                Arc::clone(&runtime_manager),
                &plan,
                strategy,
            )
            .await
            {
                Ok(pack) => {
                    info!(
                        scan_id = %scan_id,
                        strategy = ?strategy.strategy,
                        payloads = pack.stats.payload_count,
                        "generated attack payloads from payload strategy"
                    );
                    Some(prompt_payloads_map(&pack))
                }
                Err(err) => {
                    warn!(
                        scan_id = %scan_id,
                        error = %err,
                        "payload generation failed; falling back to attack builtins"
                    );
                    None
                }
            }
        } else if let Some(mode) = parse_generator_mode_optional(generator_mode.as_deref()) {
            let plan = attack_plan_from_scan(profile.clone(), categories.clone(), disabled_tests);
            match generate_payloads_for_scan_job(
                &data_dir,
                Arc::clone(&inference_manager),
                Arc::clone(&model_manager),
                model_provider.clone(),
                Arc::clone(&runtime_manager),
                &plan,
                mode,
            )
            .await
            {
                Ok(pack) => {
                    info!(
                        scan_id = %scan_id,
                        mode = ?mode,
                        payloads = pack.stats.payload_count,
                        "generated attack payloads for scan"
                    );
                    Some(prompt_payloads_map(&pack))
                }
                Err(err) => {
                    warn!(
                        scan_id = %scan_id,
                        error = %err,
                        "payload generation failed; falling back to attack builtins"
                    );
                    None
                }
            }
        } else {
            None
        };

    let total = categories.len() as u64;
    let mut findings_total = 0u64;
    let mut had_error = false;

    let profile_ref = match (&target_id, &target_profile) {
        (Some(tid), Some(p)) if p.is_verified() => Some((tid.as_str(), p)),
        _ => {
            warn!(scan_id = %scan_id, "target profile missing or not verified");
            had_error = true;
            None
        }
    };

    if let Some((tid, target_profile)) = profile_ref {
        for category in &categories {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            wait_if_paused(&paused, &cancel).await;
            if cancel.load(Ordering::Relaxed) {
                break;
            }

            {
                if let Ok(mut p) = progress.lock() {
                    p.status = if paused.load(Ordering::Relaxed) {
                        "paused".into()
                    } else {
                        "running".into()
                    };
                    p.current_endpoint = Some(target_profile.full_url());
                    p.current_test = Some(category.display_name().to_string());
                }
            }

            let inference = inference_manager.lock().await;
            let manager = model_manager.lock().await;
            let mut runtime_mgr = runtime_manager.lock().await;
            match run_category_on_target_profile(
                &repos,
                &scan_id,
                &project_id,
                tid,
                target_profile,
                *category,
                attack_runtime.clone(),
                &data_dir,
                &inference,
                &manager,
                model_provider.clone(),
                &mut runtime_mgr,
                plugin_manager.clone(),
                generated_payloads.as_ref(),
                Some(&progress_emitter),
            )
            .await
            {
                Ok(result) => {
                    findings_total += result.findings.len() as u64;
                    if let Ok(mut p) = progress.lock() {
                        p.completed += 1;
                        p.findings = findings_total;
                    }
                }
                Err(err) => {
                    warn!(
                        scan_id = %scan_id,
                        target_id = %tid,
                        category = %category.as_str(),
                        error = %err,
                        "attack unit failed"
                    );
                    had_error = true;
                    if let Ok(mut p) = progress.lock() {
                        p.completed += 1;
                    }
                }
            }

            let snapshot = progress.lock().ok().map(|guard| guard.clone());
            if let Some(snapshot) = snapshot {
                let _ = persist_playbook_progress(&repos, &scan_id, &snapshot).await;
            }
        }
    }

    let cancelled = cancel.load(Ordering::Relaxed);
    let final_status = if cancelled {
        "cancelled"
    } else if had_error && findings_total == 0 && total > 0 {
        "failed"
    } else {
        "completed"
    };

    if let Ok(mut p) = progress.lock() {
        p.status = final_status.into();
        p.current_endpoint = None;
        p.current_test = None;
        p.completed = p.completed.min(total);
    }

    let _ = repos
        .scans()
        .update(
            &scan_id,
            UpdateScan {
                status: Some(final_status.into()),
                completed_at: Some(Some(OffsetDateTime::now_utc())),
                ..Default::default()
            },
        )
        .await;

    let snapshot = progress.lock().ok().map(|guard| guard.clone());
    if let Some(snapshot) = snapshot {
        let _ = persist_playbook_progress(&repos, &scan_id, &snapshot).await;
    }

    jobs.remove(&scan_id);
    emit_app_data_changed(&app, "scan_completed");
    info!(scan_id = %scan_id, status = final_status, findings = findings_total, "scan job finished");
}

async fn run_agent_scan_job(
    app: AppHandle,
    db: aisec_storage::Database,
    jobs: ScanJobManager,
    scan_id: String,
    project_id: String,
    target_id: Option<String>,
    endpoint_ids: Vec<String>,
    categories: Vec<AttackCategory>,
    disabled_tests: Vec<String>,
    profile: String,
    generator_mode: Option<String>,
    max_agent_attempts: Option<usize>,
    cancel: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    progress: Arc<Mutex<ScanProgress>>,
    data_dir: std::path::PathBuf,
    auth_config: AuthEngineConfig,
    harness_factory: aisec_harness::HarnessFactory,
    plugin_manager: Arc<AsyncMutex<aisec_plugin_host::PluginManager>>,
    inference_manager: Arc<AsyncMutex<aisec_inference::InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<aisec_runtime::RuntimeManager>>,
) {
    let repos = db.repositories();
    let progress_emitter = ScanProgressEmitter::new(app.clone(), scan_id.clone());
    progress_emitter.info("Loading attack plan...");
    let config = agent_config_from_scan(generator_mode.as_deref(), max_agent_attempts);
    let default_runtime = fallback_attack_runtime();
    let attack_runtime: AttackRuntime = if let Some(tid) = &target_id {
        if let Ok(target) = repos.targets().get(tid).await {
            let probe_url = seed_url_from_descriptor(&target.descriptor_json)
                .unwrap_or_else(|| "https://localhost".into());
            build_attack_runtime_parts(
                db.clone(),
                &data_dir,
                auth_config.clone(),
                &harness_factory,
                plugin_manager.clone(),
                &target.descriptor_json,
                &probe_url,
            )
            .await
            .unwrap_or(default_runtime)
        } else {
            default_runtime
        }
    } else {
        default_runtime
    };

    let max_attempts = config.max_attempts_per_category as u64;
    let total = (endpoint_ids.len() as u64)
        * (categories.len() as u64)
        * max_attempts.max(1);
    {
        if let Ok(mut p) = progress.lock() {
            p.total = total;
            p.agent_mode = true;
        }
    }

    let mut findings_total = 0u64;
    let mut had_error = false;
    let completed_units = Arc::new(Mutex::new(0u64));
    let findings_arc = Arc::new(Mutex::new(0u64));

    for endpoint_id in &endpoint_ids {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        wait_if_paused(&paused, &cancel).await;
        if cancel.load(Ordering::Relaxed) {
            break;
        }

        let endpoint = match repos.endpoints().get(endpoint_id).await {
            Ok(endpoint) => endpoint,
            Err(err) => {
                warn!(scan_id = %scan_id, endpoint_id = %endpoint_id, error = %err, "endpoint lookup failed");
                had_error = true;
                continue;
            }
        };

        if let Ok(mut p) = progress.lock() {
            p.current_endpoint = Some(endpoint.url.clone());
            p.current_test = Some("agent: fingerprint".into());
            p.current_phase = Some("fingerprint".into());
        }

        let mut host = ScanAgentHost {
            repos: &repos,
            scan_id: scan_id.clone(),
            project_id: project_id.clone(),
            target_id: target_id.clone(),
            endpoint: endpoint.clone(),
            runtime: attack_runtime.clone(),
            data_dir: &data_dir,
            inference_manager: inference_manager.clone(),
            model_manager_arc: model_manager.clone(),
            model_provider: model_provider.clone(),
            runtime_manager_arc: runtime_manager.clone(),
            plugin_manager: plugin_manager.clone(),
            profile: profile.clone(),
            disabled_tests: disabled_tests.clone(),
            allowed_categories: categories.clone(),
            cancel: cancel.clone(),
            progress: progress.clone(),
            completed_units: completed_units.clone(),
            findings_total: findings_arc.clone(),
            progress_emitter: Some(progress_emitter.clone()),
            planner_mode: config.planner_mode,
        };

        match run_agent_endpoint(&mut host, &config).await {
            Ok(result) => {
                findings_total = *findings_arc.lock().unwrap();
                info!(
                    scan_id = %scan_id,
                    endpoint_id = %endpoint_id,
                    findings = result.findings,
                    attempts = result.total_attempts,
                    "agent endpoint episode completed"
                );
            }
            Err(err) => {
                warn!(
                    scan_id = %scan_id,
                    endpoint_id = %endpoint_id,
                    error = %err,
                    "agent endpoint episode failed"
                );
                had_error = true;
            }
        }

        let snapshot = progress.lock().ok().map(|guard| guard.clone());
        if let Some(snapshot) = snapshot {
            let _ = persist_playbook_progress(&repos, &scan_id, &snapshot).await;
        }
    }

    let cancelled = cancel.load(Ordering::Relaxed);
    let final_status = if cancelled {
        "cancelled"
    } else if had_error && findings_total == 0 && total > 0 {
        "failed"
    } else {
        "completed"
    };

    if let Ok(mut p) = progress.lock() {
        p.status = final_status.into();
        p.current_endpoint = None;
        p.current_test = None;
        p.current_phase = None;
        p.current_attempt = None;
        p.current_retry = None;
        p.completed = p.completed.min(total);
    }

    let _ = repos
        .scans()
        .update(
            &scan_id,
            UpdateScan {
                status: Some(final_status.into()),
                completed_at: Some(Some(OffsetDateTime::now_utc())),
                ..Default::default()
            },
        )
        .await;

    let snapshot = progress.lock().ok().map(|guard| guard.clone());
    if let Some(snapshot) = snapshot {
        let _ = persist_playbook_progress(&repos, &scan_id, &snapshot).await;
    }

    jobs.remove(&scan_id);
    emit_app_data_changed(&app, "scan_completed");
    info!(
        scan_id = %scan_id,
        status = final_status,
        findings = findings_total,
        "agent scan job finished"
    );
}

#[instrument(skip(state, app))]
pub async fn scan_start_op(
    state: &AppState,
    app: &AppHandle,
    project_id: String,
    target_id: String,
    profile: String,
    categories: Vec<String>,
    disabled_tests: Vec<String>,
    generator_mode: Option<String>,
    payload_strategy: Option<PayloadStrategy>,
    agent_mode: Option<bool>,
    max_agent_attempts: Option<usize>,
    draft_scan_id: Option<String>,
) -> CommandResult<ScanStartDto> {
    let parsed_categories: Vec<AttackCategory> = categories
        .iter()
        .filter_map(|value| parse_category(value))
        .collect();

    if parsed_categories.is_empty() {
        return Err(CommandError::invalid_input(
            "At least one valid attack category is required",
        ));
    }

    let repos = state.repositories();
    repos
        .projects()
        .get(&project_id)
        .await
        .map_err(CommandError::from)?;
    let target = repos
        .targets()
        .get(&target_id)
        .await
        .map_err(CommandError::from)?;

    let target_profile = parse_target_profile(&target.profile_json)?;
    if !target_profile.is_verified() {
        return Err(CommandError::invalid_input(
            "Target profile must be verified before starting a scan",
        ));
    }

    let agentic = agent_mode.unwrap_or(false);
    let effective_generator_mode = payload_strategy
        .as_ref()
        .map(|s| s.generator_mode_str().to_string())
        .or(generator_mode.clone());
    let config = agent_config_from_scan(effective_generator_mode.as_deref(), max_agent_attempts);
    let scan_name = if agentic {
        format!("Agent Scan ({profile})")
    } else {
        format!("Scan ({profile})")
    };

    let mut execution_playbook = {
        let mut playbook = serde_json::json!({
            "profile": profile,
            "categories": categories,
            "disabled_tests": disabled_tests,
            "target_profile": true,
            "generator_mode": effective_generator_mode,
            "agent_mode": agentic,
            "max_agent_attempts": config.max_attempts_per_category,
        });
        if let Some(ref strategy) = payload_strategy {
            playbook["payload_strategy"] = serde_json::to_value(strategy).unwrap_or_default();
            playbook["scan_metadata"] = serde_json::json!({
                "generation_strategy": match strategy.strategy {
                    aisec_target_profile::PayloadGenerationStrategy::Deterministic => "deterministic",
                    aisec_target_profile::PayloadGenerationStrategy::Mutation => "mutation",
                    aisec_target_profile::PayloadGenerationStrategy::Adaptive => "adaptive",
                },
                "mutation_level": match strategy.mutation_level {
                    aisec_target_profile::MutationLevel::Low => "low",
                    aisec_target_profile::MutationLevel::Medium => "medium",
                    aisec_target_profile::MutationLevel::High => "high",
                    aisec_target_profile::MutationLevel::Extreme => "extreme",
                },
                "variant_count": strategy.variants_per_test,
                "adaptive_enabled": strategy.enable_response_adaptation,
                "context_enabled": strategy.enable_context_awareness,
                "conversation_enabled": strategy.enable_conversation_memory,
                "payload_budget": strategy.max_total_payloads,
            });
        }
        playbook
    };

    let scan = if let Some(draft_id) = draft_scan_id.filter(|id| !id.trim().is_empty()) {
        let existing = repos
            .scans()
            .get(&draft_id)
            .await
            .map_err(CommandError::from)?;
        if existing.project_id != project_id {
            return Err(CommandError::invalid_input("draft scan project mismatch"));
        }

        let is_draft = existing.status == crate::commands::wizard_scan::WIZARD_SCAN_STATUS;
        let is_restart = is_restartable_scan_status(&existing.status);

        if !is_draft && !is_restart {
            if existing.status == "running" && state.jobs().contains(&draft_id) {
                return Err(CommandError::invalid_input("scan is already running"));
            }
            if existing.status == "paused" {
                return Err(CommandError::invalid_input(
                    "scan is paused; resume it instead of starting a new run",
                ));
            }
            if existing.status != "running" {
                return Err(CommandError::invalid_input(format!(
                    "scan cannot be restarted from status '{}'",
                    existing.status
                )));
            }
            warn!(scan_id = %draft_id, "restarting scan marked running without active job");
        }

        let playbook = merge_scan_execution_playbook(
            existing.playbook_json.as_deref(),
            &mut execution_playbook,
            is_restart,
        );

        let mut update = UpdateScan {
            target_id: Some(Some(target_id.clone())),
            name: Some(scan_name.clone()),
            status: Some("running".into()),
            playbook_json: Some(playbook),
            started_at: Some(Some(OffsetDateTime::now_utc())),
            ..Default::default()
        };
        if is_restart {
            update.completed_at = Some(None);
        }

        repos
            .scans()
            .update(&draft_id, update)
            .await
            .map_err(CommandError::from)?
    } else {
        repos
            .scans()
            .create(CreateScan {
                project_id: project_id.clone(),
                target_id: Some(target_id.clone()),
                name: scan_name.clone(),
                status: Some("running".into()),
                playbook_json: Some(execution_playbook),
            })
            .await
            .map_err(CommandError::from)?
    };

    if scan.started_at.is_none() {
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
    }

    let total = if agentic {
        (parsed_categories.len() as u64) * config.max_attempts_per_category as u64
    } else {
        parsed_categories.len() as u64
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let paused = Arc::new(AtomicBool::new(false));
    let mut progress_state = ScanProgress::new(total.max(1));
    progress_state.agent_mode = agentic;
    let progress = Arc::new(Mutex::new(progress_state));
    state.jobs().register(
        scan.id.clone(),
        cancel.clone(),
        paused.clone(),
        progress.clone(),
    );

    let db = state.database().clone();
    let jobs = state.jobs().clone();
    let scan_id = scan.id.clone();
    let data_dir = state.data_dir().to_path_buf();
    let auth_config = state.auth_engine_config().clone();
    let harness_factory = state.harness_factory().clone();
    let plugin_manager = Arc::clone(state.plugin_manager());
    let inference_manager = Arc::clone(state.inference_manager());
    let model_manager = Arc::clone(state.model_manager());
    let model_provider = state.model_provider().clone();
    let runtime_manager = Arc::clone(state.runtime_manager());

    let disabled_for_job = disabled_tests.clone();
    let profile_for_job = profile.clone();
    let generator_mode_for_job = effective_generator_mode.clone();
    let payload_strategy_for_job = payload_strategy.clone();
    let max_attempts_for_job = max_agent_attempts;
    let app_for_job = app.clone();

    emit_app_data_changed(app, "scan_created");

    if agentic {
        warn!(
            scan_id = %scan.id,
            "agent mode runs as target-profile batch scan until agent adapter migrates"
        );
    }

    tauri::async_runtime::spawn(async move {
        run_scan_job(
            app_for_job,
            db,
            jobs,
            scan_id,
            project_id,
            Some(target_id),
            parsed_categories,
            disabled_for_job,
            profile_for_job,
            generator_mode_for_job,
            payload_strategy_for_job,
            cancel,
            paused,
            progress,
            data_dir,
            auth_config,
            harness_factory,
            plugin_manager,
            inference_manager,
            model_manager,
            model_provider,
            runtime_manager,
        )
        .await;
    });

    info!(
        scan_id = %scan.id,
        total_units = total,
        agentic = agentic,
        "scan started in background"
    );
    Ok(ScanStartDto {
        scan_id: scan.id,
    })
}

pub async fn scan_status_op(state: &AppState, scan_id: String) -> CommandResult<ScanStatusDto> {
    if let Some(progress) = state.jobs().progress(&scan_id) {
        return Ok(progress_to_dto(&scan_id, &progress));
    }

    let repos = state.repositories();
    let scan = repos
        .scans()
        .get(&scan_id)
        .await
        .map_err(CommandError::from)?;

    if let Some(progress) = progress_from_playbook(scan.playbook_json.as_deref()) {
        return Ok(progress_to_dto(&scan_id, &progress));
    }

    let findings_count = repos
        .findings()
        .list_by_scan(&scan_id)
        .await
        .map_err(CommandError::from)?
        .len() as u64;

    Ok(ScanStatusDto {
        scan_id,
        status: scan.status.clone(),
        progress_percent: if scan.status == "completed" {
            100.0
        } else {
            0.0
        },
        completed: 0,
        total: 0,
        findings_count,
        current_endpoint: None,
        current_test: None,
        started_at: scan.started_at.map(|dt| {
            dt.format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default()
        }),
        agent_mode: false,
        current_phase: None,
        current_attempt: None,
        current_retry: None,
    })
}

pub async fn scan_pause_op(state: &AppState, scan_id: String) -> CommandResult<ScanStatusDto> {
    let current = state
        .jobs()
        .progress(&scan_id)
        .ok_or_else(|| CommandError::invalid_input("Scan is not running"))?;
    if current.status == "paused" {
        return scan_status_op(state, scan_id).await;
    }
    if current.status != "running" {
        return Err(CommandError::invalid_input("Scan is not running"));
    }

    if !state.jobs().set_paused(&scan_id, true) {
        return Err(CommandError::invalid_input("Scan is not running"));
    }

    let _ = state
        .repositories()
        .scans()
        .update(
            &scan_id,
            UpdateScan {
                status: Some("paused".into()),
                ..Default::default()
            },
        )
        .await;

    if let Some(progress) = state.jobs().progress(&scan_id) {
        let _ = persist_playbook_progress(&state.repositories(), &scan_id, &progress).await;
    }

    scan_status_op(state, scan_id).await
}

pub async fn scan_resume_op(state: &AppState, scan_id: String) -> CommandResult<ScanStatusDto> {
    let current = state
        .jobs()
        .progress(&scan_id)
        .ok_or_else(|| CommandError::invalid_input("Scan is not paused"))?;
    if current.status != "paused" {
        return Err(CommandError::invalid_input("Scan is not paused"));
    }

    if !state.jobs().set_paused(&scan_id, false) {
        return Err(CommandError::invalid_input("Scan is not paused"));
    }

    let _ = state
        .repositories()
        .scans()
        .update(
            &scan_id,
            UpdateScan {
                status: Some("running".into()),
                ..Default::default()
            },
        )
        .await;

    if let Some(progress) = state.jobs().progress(&scan_id) {
        let _ = persist_playbook_progress(&state.repositories(), &scan_id, &progress).await;
    }

    scan_status_op(state, scan_id).await
}

pub async fn scan_stop_op(state: &AppState, scan_id: String) -> CommandResult<ScanStatusDto> {
    if !state.jobs().request_cancel(&scan_id) {
        return Err(CommandError::invalid_input("Scan is not active"));
    }

    let _ = state
        .repositories()
        .scans()
        .update(
            &scan_id,
            UpdateScan {
                status: Some("cancelled".into()),
                completed_at: Some(Some(OffsetDateTime::now_utc())),
                ..Default::default()
            },
        )
        .await;

    scan_status_op(state, scan_id).await
}

#[tauri::command]
pub async fn scan_start(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    target_id: String,
    profile: String,
    categories: Vec<String>,
    disabled_tests: Vec<String>,
    generator_mode: Option<String>,
    payload_strategy: Option<serde_json::Value>,
    agent_mode: Option<bool>,
    max_agent_attempts: Option<usize>,
    draft_scan_id: Option<String>,
) -> CommandResult<ScanStartDto> {
    let parsed_strategy = payload_strategy
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| CommandError::invalid_input(format!("invalid payload strategy: {e}")))?;
    scan_start_op(
        state.inner(),
        &app,
        project_id,
        target_id,
        profile,
        categories,
        disabled_tests,
        generator_mode,
        parsed_strategy,
        agent_mode,
        max_agent_attempts,
        draft_scan_id,
    )
    .await
}

#[tauri::command]
pub async fn scan_status(
    state: State<'_, AppState>,
    scan_id: String,
) -> CommandResult<ScanStatusDto> {
    scan_status_op(state.inner(), scan_id).await
}

#[tauri::command]
pub async fn scan_pause(
    state: State<'_, AppState>,
    scan_id: String,
) -> CommandResult<ScanStatusDto> {
    scan_pause_op(state.inner(), scan_id).await
}

#[tauri::command]
pub async fn scan_resume(
    state: State<'_, AppState>,
    scan_id: String,
) -> CommandResult<ScanStatusDto> {
    scan_resume_op(state.inner(), scan_id).await
}

#[tauri::command]
pub async fn scan_stop(state: State<'_, AppState>, scan_id: String) -> CommandResult<ScanStatusDto> {
    scan_stop_op(state.inner(), scan_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restartable_scan_statuses_include_terminal_states() {
        assert!(is_restartable_scan_status("failed"));
        assert!(is_restartable_scan_status("cancelled"));
        assert!(is_restartable_scan_status("stopped"));
        assert!(is_restartable_scan_status("completed"));
        assert!(!is_restartable_scan_status("draft"));
        assert!(!is_restartable_scan_status("running"));
    }

    #[test]
    fn merge_scan_execution_playbook_preserves_wizard_and_clears_progress_on_restart() {
        let existing = serde_json::json!({
            "wizard": { "step": 4 },
            "progress": { "completed": 3, "total": 5 }
        });
        let mut execution = serde_json::json!({
            "profile": "standard",
            "categories": ["prompt_injection"]
        });

        let merged = merge_scan_execution_playbook(
            Some(&existing.to_string()),
            &mut execution,
            true,
        );

        assert_eq!(merged["wizard"], serde_json::json!({ "step": 4 }));
        assert_eq!(merged["profile"], "standard");
        assert!(merged.get("progress").is_none());
        assert_eq!(
            execution["wizard_snapshot"],
            serde_json::json!({ "step": 4 })
        );
    }
}
