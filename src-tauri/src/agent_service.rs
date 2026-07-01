//! Tauri bridge implementing [`aisec_agent::AgentHost`] for background scans.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use aisec_agent::{
    AgentConfig, AgentError, AgentHost, AgentPhase, AgentResult, AgentScanResult, AgentVerdict,
    AttackExecutionSummary, run_endpoint_agent,
};
use aisec_attack::{AttackCategory, AttackPayload};
use aisec_fingerprint::StackFingerprintReport;
use aisec_generator::{
    generate_from_plan, GeneratorMode, PromptPayloads,
};
use aisec_planner::{generate_attack_plan, AttackPlan, FingerprintResult, PlannerMode};
use aisec_inference::InferenceRuntimeManager;
use aisec_runtime::SharedModelProvider;
use aisec_storage::{Endpoint, Repositories};
use async_trait::async_trait;
use tauri::async_runtime::Mutex as AsyncMutex;
use crate::commands::attack::run_category_on_endpoint;
use crate::jobs::ScanProgress;
use crate::events::ScanProgressEmitter;
use crate::session_auth::AttackRuntime;
use crate::inference_host::{build_judge_engine_from_gateway, is_inference_ready, HostGeneratorLlm, HostPlannerLlm};

pub struct ScanAgentHost<'a> {
    pub repos: &'a Repositories,
    pub scan_id: String,
    pub project_id: String,
    pub target_id: Option<String>,
    pub endpoint: Endpoint,
    pub runtime: AttackRuntime,
    pub data_dir: &'a Path,
    pub inference_manager: Arc<AsyncMutex<InferenceRuntimeManager>>,
    pub model_manager_arc: Arc<AsyncMutex<aisec_models::LocalModelManager>>,
    pub model_provider: SharedModelProvider,
    pub runtime_manager_arc: Arc<AsyncMutex<aisec_runtime::RuntimeManager>>,
    pub plugin_manager: Arc<AsyncMutex<aisec_plugin_host::PluginManager>>,
    pub profile: String,
    pub disabled_tests: Vec<String>,
    pub allowed_categories: Vec<AttackCategory>,
    pub cancel: Arc<AtomicBool>,
    pub progress: Arc<Mutex<ScanProgress>>,
    pub completed_units: Arc<Mutex<u64>>,
    pub findings_total: Arc<Mutex<u64>>,
    pub progress_emitter: Option<ScanProgressEmitter>,
    pub planner_mode: PlannerMode,
}

#[async_trait]
impl AgentHost for ScanAgentHost<'_> {
    async fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    async fn load_fingerprint(
        &self,
        endpoint_id: &str,
        url: &str,
    ) -> AgentResult<StackFingerprintReport> {
        if endpoint_id != self.endpoint.id {
            return Err(AgentError::InvalidInput("endpoint mismatch".into()));
        }
        let report = crate::dto::stack_fingerprint_from_endpoint(&self.endpoint).ok_or_else(|| {
            AgentError::InvalidInput(format!(
                "endpoint {url} has no AI metadata — re-run discovery first"
            ))
        })?;
        Ok(report)
    }

    async fn on_phase(
        &mut self,
        phase: AgentPhase,
        detail: &str,
        attempt: u32,
        retry: u32,
    ) {
        if let Ok(mut progress) = self.progress.lock() {
            progress.current_test = Some(format!(
                "{} · {} (try {attempt})",
                phase.as_str(),
                detail.chars().take(48).collect::<String>()
            ));
            progress.current_phase = Some(phase.as_str().into());
            progress.current_attempt = Some(attempt);
            progress.current_retry = Some(retry);
        }
    }

    async fn plan(&mut self, fingerprint: &FingerprintResult) -> AgentResult<AttackPlan> {
        let mode = if self.planner_mode == PlannerMode::LocalLlm {
            let inference = self.inference_manager.lock().await;
            if is_inference_ready(&inference) {
                PlannerMode::LocalLlm
            } else {
                PlannerMode::Deterministic
            }
        } else {
            self.planner_mode
        };

        let llm = if mode == PlannerMode::LocalLlm {
            Some(Arc::new(HostPlannerLlm::new(
                self.data_dir.to_path_buf(),
                self.inference_manager.clone(),
                self.model_manager_arc.clone(),
                self.model_provider.clone(),
                self.runtime_manager_arc.clone(),
            )) as Arc<dyn aisec_planner::PlannerLlm>)
        } else {
            None
        };

        generate_attack_plan(
            fingerprint,
            mode,
            llm.as_ref().map(|adapter| adapter.as_ref()),
        )
        .await
        .map_err(Into::into)
    }

    async fn generate_payloads(
        &mut self,
        plan: &AttackPlan,
        category: AttackCategory,
        mode: GeneratorMode,
    ) -> AgentResult<PromptPayloads> {
        let category_plan = AttackPlan {
            categories: vec![category],
            ..plan.clone()
        };

        let effective_mode = if mode == GeneratorMode::LocalLlm {
            let inference = self.inference_manager.lock().await;
            if is_inference_ready(&inference) {
                GeneratorMode::LocalLlm
            } else {
                GeneratorMode::StaticPack
            }
        } else {
            GeneratorMode::StaticPack
        };

        let llm = if effective_mode == GeneratorMode::LocalLlm {
            Some(Arc::new(HostGeneratorLlm::new(
                self.data_dir.to_path_buf(),
                self.inference_manager.clone(),
                self.model_manager_arc.clone(),
                self.model_provider.clone(),
                self.runtime_manager_arc.clone(),
            )) as Arc<dyn aisec_generator::GeneratorLlm>)
        } else {
            None
        };

        generate_from_plan(
            &category_plan,
            effective_mode,
            llm.as_ref().map(|adapter| adapter.as_ref()),
        )
        .await
        .map_err(Into::into)
    }

    async fn execute_attack(
        &mut self,
        category: AttackCategory,
        payloads: &PromptPayloads,
    ) -> AgentResult<AttackExecutionSummary> {
        let map = payload_map_for_category(payloads, category);
        let run = run_category_on_endpoint(
            self.repos,
            &self.scan_id,
            &self.project_id,
            self.target_id.clone(),
            &self.endpoint,
            category,
            self.runtime.clone(),
            self.data_dir,
            Arc::clone(&self.inference_manager),
            Arc::clone(&self.model_manager_arc),
            self.model_provider.clone(),
            Arc::clone(&self.runtime_manager_arc),
            self.plugin_manager.clone(),
            Some(&map),
            self.progress_emitter.as_ref(),
            None,
            None,
        )
        .await
        .map_err(|err| AgentError::Attack(err.to_string()))?;

        {
            let mut findings = self.findings_total.lock().unwrap();
            *findings += run.findings.len() as u64;
        }
        {
            let mut completed = self.completed_units.lock().unwrap();
            *completed += 1;
        }
        if let Ok(mut progress) = self.progress.lock() {
            progress.completed = *self.completed_units.lock().unwrap();
            progress.findings = *self.findings_total.lock().unwrap();
        }

        Ok(AttackExecutionSummary {
            category,
            attempts: run.attempts,
            verdicts: run
                .judged
                .into_iter()
                .map(|item| AgentVerdict {
                    payload_id: item.payload_id,
                    payload_name: item.payload_name,
                    vulnerable: item.vulnerable,
                    confidence: item.confidence,
                    summary: item.summary,
                })
                .collect(),
        })
    }

    async fn evaluate_attack(
        &mut self,
        category: AttackCategory,
        execution: &AttackExecutionSummary,
    ) -> AgentResult<AttackExecutionSummary> {
        let _ = category;
        Ok(execution.clone())
    }
}

fn payload_map_for_category(
    payloads: &PromptPayloads,
    category: AttackCategory,
) -> HashMap<AttackCategory, Vec<AttackPayload>> {
    let mut map = HashMap::new();
    if let Some(items) = payloads.by_category.get(&category) {
        map.insert(category, items.clone());
    }
    payloads.by_category.clone()
}

pub fn agent_config_from_scan(
    generator_mode: Option<&str>,
    max_attempts: Option<usize>,
) -> AgentConfig {
    let initial_generator_mode = generator_mode
        .map(|raw| match raw.trim().to_ascii_lowercase().as_str() {
            "template_mutation" | "mutation" | "template" => GeneratorMode::TemplateMutation,
            "local_llm" | "local" | "llm" => GeneratorMode::LocalLlm,
            _ => GeneratorMode::StaticPack,
        })
        .unwrap_or(GeneratorMode::StaticPack);

    let planner_mode = if initial_generator_mode == GeneratorMode::LocalLlm {
        PlannerMode::LocalLlm
    } else {
        PlannerMode::Deterministic
    };

    AgentConfig {
        max_attempts_per_category: max_attempts.unwrap_or(5),
        planner_mode,
        initial_generator_mode,
    }
}

pub async fn run_agent_endpoint(
    host: &mut ScanAgentHost<'_>,
    config: &AgentConfig,
) -> AgentResult<AgentScanResult> {
    host.planner_mode = config.planner_mode;
    let endpoint_id = host.endpoint.id.clone();
    let endpoint_url = host.endpoint.url.clone();
    let allowed = host.allowed_categories.clone();
    run_endpoint_agent(host, config, &endpoint_id, &endpoint_url, &allowed).await
}
