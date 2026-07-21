//! Desktop bridge to [`aisec_inference::AiInferenceGateway`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use aisec_auth::SecretStore;
use aisec_core::AisecError;
use aisec_inference::{
    CompleteRequest, ConnectivityTestResult, DefaultAiInferenceGateway, GatewaySession,
    InferenceMode, InferenceProvider, InferenceRuntimeManager, InferenceSession,
    PromptRegistry, RemoteAdapterSettings,
};
use aisec_judge::{build_judge_engine_with_adapter, JudgeEngine, JudgeMode, JudgeProviderConfig};
use aisec_models::{BuiltinCatalog, LocalModelManager, ModelEntry, ModelProvider};
use aisec_planner::PlannerLlm;
use aisec_generator::GeneratorLlm;
use aisec_runtime::{RuntimeManager, SharedModelProvider};
use async_trait::async_trait;
use tauri::async_runtime::Mutex as AsyncMutex;

use crate::error::{CommandError, CommandResult};
use crate::third_party_credentials::{
    open_model_credential_vault, resolve_third_party_credentials, ThirdPartyCredentialFields,
};

pub fn models_vault_path(data_dir: &Path) -> PathBuf {
    data_dir.join("models")
}

pub fn open_model_manager(
    data_dir: &Path,
    catalog: BuiltinCatalog,
) -> CommandResult<LocalModelManager> {
    LocalModelManager::new(models_vault_path(data_dir))
        .map(|mgr| mgr.with_catalog(catalog))
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))
}

pub async fn resolve_remote_settings(
    data_dir: &Path,
    entry: &ModelEntry,
) -> CommandResult<RemoteAdapterSettings> {
    let vault = open_model_credential_vault(data_dir)?;
    let secrets = SecretStore::new().map_err(CommandError::from)?;
    let mut credentials = ThirdPartyCredentialFields::default();
    resolve_third_party_credentials(
        &mut credentials,
        Some(&entry.metadata),
        &vault,
        &secrets,
    )?;
    InferenceRuntimeManager::remote_settings_from_entry(
        entry,
        credentials.api_key,
        Some(credentials.aws_secret_access_key).filter(|s| !s.trim().is_empty()),
        Some(credentials.aws_session_token).filter(|s| !s.trim().is_empty()),
    )
    .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))
}

fn model_entry<'a>(
    manager: &'a LocalModelManager,
    inference: &InferenceRuntimeManager,
) -> CommandResult<&'a ModelEntry> {
    let model_id = inference
        .config()
        .selected_model_id
        .as_ref()
        .ok_or_else(|| CommandError::invalid_input("AI runtime has no selected model"))?;
    manager.get_model(model_id).ok_or_else(|| {
        CommandError::invalid_input(format!("model {model_id} not found"))
    })
}

pub async fn open_gateway_session<'a>(
    data_dir: &Path,
    inference: &'a InferenceRuntimeManager,
    model_manager: &'a LocalModelManager,
    model_provider: SharedModelProvider,
    runtime_manager: &'a mut RuntimeManager,
) -> CommandResult<GatewaySession<'a>> {
    let entry = model_entry(model_manager, inference)?;
    let remote = if entry.provider == ModelProvider::Remote {
        Some(resolve_remote_settings(data_dir, entry).await?)
    } else {
        None
    };
    Ok(GatewaySession {
        inner: InferenceSession {
            manager: inference,
            runtime_manager,
            model_provider,
            model_entry: entry,
            remote_settings: remote,
        },
    })
}

pub async fn build_judge_engine_from_gateway(
    data_dir: &Path,
    inference: &InferenceRuntimeManager,
    model_manager: &LocalModelManager,
    model_provider: SharedModelProvider,
    runtime_manager: &mut RuntimeManager,
) -> CommandResult<JudgeEngine> {
    if !inference.is_ready() {
        return Err(CommandError::invalid_input(
            "AI runtime must be ready before judging scan results — configure and start AI Runtime",
        ));
    }
    let entry = model_entry(model_manager, inference)?;
    let mut session = open_gateway_session(
        data_dir,
        inference,
        model_manager,
        model_provider,
        runtime_manager,
    )
    .await?;
    let adapter = DefaultAiInferenceGateway::adapter_for(&mut session.inner)
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
    let mut config = JudgeProviderConfig::default();
    config.mode = if entry.provider == ModelProvider::Remote {
        JudgeMode::RemoteLlm
    } else {
        JudgeMode::LocalLlm
    };
    build_judge_engine_with_adapter(adapter, &config)
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))
}

pub async fn gateway_complete(
    data_dir: &Path,
    inference: &InferenceRuntimeManager,
    model_manager: &LocalModelManager,
    model_provider: SharedModelProvider,
    runtime_manager: &mut RuntimeManager,
    system: Option<&str>,
    prompt: &str,
    max_tokens: u32,
    temperature: f32,
) -> CommandResult<String> {
    let mut session = open_gateway_session(
        data_dir,
        inference,
        model_manager,
        model_provider,
        runtime_manager,
    )
    .await?;
    session
        .complete(CompleteRequest {
            prompt: prompt.to_string(),
            system: system.map(String::from),
            max_tokens: Some(max_tokens),
            temperature: Some(temperature),
        })
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))
}

/// Wizard attack-plan LLM — higher token budget and JSON-focused system prompt.
pub struct HostWizardPlannerLlm {
    data_dir: PathBuf,
    inference: Arc<AsyncMutex<InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
}

impl HostWizardPlannerLlm {
    pub fn new(
        data_dir: PathBuf,
        inference: Arc<AsyncMutex<InferenceRuntimeManager>>,
        model_manager: Arc<AsyncMutex<LocalModelManager>>,
        model_provider: SharedModelProvider,
        runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    ) -> Self {
        Self {
            data_dir,
            inference,
            model_manager,
            model_provider,
            runtime_manager,
        }
    }
}

#[async_trait]
impl PlannerLlm for HostWizardPlannerLlm {
    async fn complete(&self, prompt: &str) -> aisec_planner::PlannerResult<String> {
        let inference = self.inference.lock().await;
        let manager = self.model_manager.lock().await;
        let mut runtime_mgr = self.runtime_manager.lock().await;
        gateway_complete(
            &self.data_dir,
            &inference,
            &manager,
            self.model_provider.clone(),
            &mut runtime_mgr,
            Some(PromptRegistry::wizard_profile_system()),
            prompt,
            8192,
            0.1,
        )
        .await
        .map_err(|e| aisec_planner::PlannerError::Llm(e.to_string()))
    }
}

/// Endpoint verify LLM — classifies whether a probe response is from an AI API.
pub struct HostEndpointVerifyLlm {
    data_dir: PathBuf,
    inference: Arc<AsyncMutex<InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
}

impl HostEndpointVerifyLlm {
    pub fn new(
        data_dir: PathBuf,
        inference: Arc<AsyncMutex<InferenceRuntimeManager>>,
        model_manager: Arc<AsyncMutex<LocalModelManager>>,
        model_provider: SharedModelProvider,
        runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    ) -> Self {
        Self {
            data_dir,
            inference,
            model_manager,
            model_provider,
            runtime_manager,
        }
    }
}

#[async_trait]
impl PlannerLlm for HostEndpointVerifyLlm {
    async fn complete(&self, prompt: &str) -> aisec_planner::PlannerResult<String> {
        let inference = self.inference.lock().await;
        let manager = self.model_manager.lock().await;
        let mut runtime_mgr = self.runtime_manager.lock().await;
        gateway_complete(
            &self.data_dir,
            &inference,
            &manager,
            self.model_provider.clone(),
            &mut runtime_mgr,
            Some(PromptRegistry::endpoint_verify_system()),
            prompt,
            1024,
            0.1,
        )
        .await
        .map_err(|e| aisec_planner::PlannerError::Llm(e.to_string()))
    }
}

/// Attack results recommendation LLM — remediation guidance from scan findings.
pub struct HostAttackRecommendLlm {
    data_dir: PathBuf,
    inference: Arc<AsyncMutex<InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
}

impl HostAttackRecommendLlm {
    pub fn new(
        data_dir: PathBuf,
        inference: Arc<AsyncMutex<InferenceRuntimeManager>>,
        model_manager: Arc<AsyncMutex<LocalModelManager>>,
        model_provider: SharedModelProvider,
        runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    ) -> Self {
        Self {
            data_dir,
            inference,
            model_manager,
            model_provider,
            runtime_manager,
        }
    }
}

#[async_trait]
impl PlannerLlm for HostAttackRecommendLlm {
    async fn complete(&self, prompt: &str) -> aisec_planner::PlannerResult<String> {
        let inference = self.inference.lock().await;
        let manager = self.model_manager.lock().await;
        let mut runtime_mgr = self.runtime_manager.lock().await;
        gateway_complete(
            &self.data_dir,
            &inference,
            &manager,
            self.model_provider.clone(),
            &mut runtime_mgr,
            Some(PromptRegistry::attack_results_recommend_system()),
            prompt,
            2048,
            0.15,
        )
        .await
        .map_err(|e| aisec_planner::PlannerError::Llm(e.to_string()))
    }
}

/// Project-level summary LLM — posture overview across targets, scans, and findings.
pub struct HostProjectSummaryLlm {
    data_dir: PathBuf,
    inference: Arc<AsyncMutex<InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
}

impl HostProjectSummaryLlm {
    pub fn new(
        data_dir: PathBuf,
        inference: Arc<AsyncMutex<InferenceRuntimeManager>>,
        model_manager: Arc<AsyncMutex<LocalModelManager>>,
        model_provider: SharedModelProvider,
        runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    ) -> Self {
        Self {
            data_dir,
            inference,
            model_manager,
            model_provider,
            runtime_manager,
        }
    }
}

#[async_trait]
impl PlannerLlm for HostProjectSummaryLlm {
    async fn complete(&self, prompt: &str) -> aisec_planner::PlannerResult<String> {
        let inference = self.inference.lock().await;
        let manager = self.model_manager.lock().await;
        let mut runtime_mgr = self.runtime_manager.lock().await;
        gateway_complete(
            &self.data_dir,
            &inference,
            &manager,
            self.model_provider.clone(),
            &mut runtime_mgr,
            Some(PromptRegistry::project_summary_system()),
            prompt,
            2048,
            0.15,
        )
        .await
        .map_err(|e| aisec_planner::PlannerError::Llm(e.to_string()))
    }
}

/// Generator LLM backed by the AI Inference Gateway.
pub struct HostGeneratorLlm {
    data_dir: PathBuf,
    inference: Arc<AsyncMutex<InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
}

impl HostGeneratorLlm {
    pub fn new(
        data_dir: PathBuf,
        inference: Arc<AsyncMutex<InferenceRuntimeManager>>,
        model_manager: Arc<AsyncMutex<LocalModelManager>>,
        model_provider: SharedModelProvider,
        runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    ) -> Self {
        Self {
            data_dir,
            inference,
            model_manager,
            model_provider,
            runtime_manager,
        }
    }
}

#[async_trait]
impl GeneratorLlm for HostGeneratorLlm {
    async fn complete(&self, prompt: &str) -> aisec_generator::GeneratorResult<String> {
        let inference = self.inference.lock().await;
        let manager = self.model_manager.lock().await;
        let mut runtime_mgr = self.runtime_manager.lock().await;
        gateway_complete(
            &self.data_dir,
            &inference,
            &manager,
            self.model_provider.clone(),
            &mut runtime_mgr,
            Some(PromptRegistry::generator_system()),
            prompt,
            1536,
            0.2,
        )
        .await
        .map_err(|e| aisec_generator::GeneratorError::Llm(e.to_string()))
    }
}

pub fn is_inference_ready(inference: &InferenceRuntimeManager) -> bool {
    inference.is_ready()
}

fn configure_scratch_for_entry(
    scratch: &mut InferenceRuntimeManager,
    entry: &ModelEntry,
    remote: Option<&RemoteAdapterSettings>,
) -> CommandResult<()> {
    let config = scratch.config_mut();
    config.selected_model_id = Some(entry.id.clone());
    config.model = entry.display_model_name();
    config.initialized = true;
    config.status = "configured".into();
    match entry.provider {
        ModelProvider::Remote => {
            config.mode = InferenceMode::ThirdParty;
            let remote = remote.ok_or_else(|| {
                CommandError::invalid_input("third-party model requires credentials")
            })?;
            config.provider = remote.provider;
            config.runtime = "cloud".into();
        }
        ModelProvider::Ollama => {
            config.mode = InferenceMode::Local;
            config.provider = InferenceProvider::Ollama;
            config.runtime = "ollama".into();
        }
        _ => {
            config.mode = InferenceMode::Local;
            config.provider = InferenceProvider::LlamaCpp;
            config.runtime = "llama.cpp".into();
        }
    }
    Ok(())
}

pub async fn test_remote_connectivity_only(
    entry: &ModelEntry,
    remote: RemoteAdapterSettings,
) -> CommandResult<ConnectivityTestResult> {
    use aisec_inference::{ProviderAdapter, RemoteProviderAdapter};

    let adapter = RemoteProviderAdapter::new(remote.clone());
    let started = std::time::Instant::now();
    let latency_ms = || started.elapsed().as_millis() as u64;

    aisec_inference::record_sent();
    let result = ProviderAdapter::health(&adapter).await;
    if result.is_ok() {
        aisec_inference::record_received();
    }

    match result {
        Ok(true) => Ok(ConnectivityTestResult {
            ok: true,
            provider: remote.provider.as_str().into(),
            model: entry.display_model_name(),
            latency_ms: latency_ms(),
            message: "Connection Successful".into(),
            sample_response: None,
        }),
        Ok(false) => Ok(ConnectivityTestResult {
            ok: false,
            provider: remote.provider.as_str().into(),
            model: entry.display_model_name(),
            latency_ms: latency_ms(),
            message: "Connection Failed: model returned an empty response".into(),
            sample_response: None,
        }),
        Err(err) => Ok(ConnectivityTestResult {
            ok: false,
            provider: remote.provider.as_str().into(),
            model: entry.display_model_name(),
            latency_ms: latency_ms(),
            message: format!("Connection Failed: {err}"),
            sample_response: None,
        }),
    }
}

pub async fn test_connectivity_for_entry(
    data_dir: &Path,
    entry: &ModelEntry,
    model_provider: SharedModelProvider,
    runtime_manager: &mut RuntimeManager,
) -> CommandResult<ConnectivityTestResult> {
    let remote = if entry.provider == ModelProvider::Remote {
        Some(resolve_remote_settings(data_dir, entry).await?)
    } else {
        None
    };
    test_connectivity_with_remote(data_dir, entry, remote, model_provider, runtime_manager).await
}

pub async fn test_connectivity_with_remote(
    data_dir: &Path,
    entry: &ModelEntry,
    remote: Option<RemoteAdapterSettings>,
    model_provider: SharedModelProvider,
    runtime_manager: &mut RuntimeManager,
) -> CommandResult<ConnectivityTestResult> {
    let mut scratch = InferenceRuntimeManager::new(data_dir);
    configure_scratch_for_entry(&mut scratch, entry, remote.as_ref())?;
    let mut session = GatewaySession {
        inner: InferenceSession {
            manager: &scratch,
            runtime_manager,
            model_provider,
            model_entry: entry,
            remote_settings: remote,
        },
    };
    session
        .test_connectivity()
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))
}

pub async fn test_inference_for_entry(
    data_dir: &Path,
    entry: &ModelEntry,
    model_provider: SharedModelProvider,
    runtime_manager: &mut RuntimeManager,
) -> CommandResult<ConnectivityTestResult> {
    let remote = if entry.provider == ModelProvider::Remote {
        Some(resolve_remote_settings(data_dir, entry).await?)
    } else {
        None
    };
    let mut scratch = InferenceRuntimeManager::new(data_dir);
    configure_scratch_for_entry(&mut scratch, entry, remote.as_ref())?;
    let mut session = GatewaySession {
        inner: InferenceSession {
            manager: &scratch,
            runtime_manager,
            model_provider,
            model_entry: entry,
            remote_settings: remote,
        },
    };
    session
        .test_inference()
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))
}

pub fn connectivity_to_judge(result: ConnectivityTestResult) -> aisec_judge::JudgeConnectivityResult {
    aisec_judge::JudgeConnectivityResult {
        ok: result.ok,
        provider: result.provider,
        model: result.model,
        latency_ms: result.latency_ms,
        message: result.message,
        sample_response: result.sample_response,
    }
}
