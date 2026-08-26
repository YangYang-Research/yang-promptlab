//! Desktop bridge to [`promptlab_inference::AiInferenceGateway`].

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use promptlab_harness::CancelFlag;

use promptlab_auth::SecretStore;
use promptlab_core::PromptLabError;
use promptlab_inference::{
    CompleteRequest, ConnectivityTestResult, GatewaySession,
    InferenceMode, InferenceProvider, InferenceRuntimeManager, InferenceSession,
    PromptRegistry, RemoteAdapterSettings,
};
use promptlab_judge::{build_judge_engine_with_client, JudgeEngine, JudgeMode, JudgeProviderConfig};
use promptlab_models::{LocalModelManager, ModelEntry, ModelProvider};
use promptlab_planner::PlannerLlm;
use promptlab_generator::GeneratorLlm;
use promptlab_runtime::{RuntimeManager, SharedModelProvider};
use async_trait::async_trait;
use tauri::async_runtime::Mutex as AsyncMutex;

use crate::error::{CommandError, CommandResult};
use crate::third_party_credentials::{
    open_model_credential_vault, resolve_third_party_credentials, ThirdPartyCredentialFields,
};

pub fn models_vault_path(data_dir: &Path) -> PathBuf {
    data_dir.join("models")
}

fn assistant_cancel_slot() -> &'static Mutex<CancelFlag> {
    static SLOT: OnceLock<Mutex<CancelFlag>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(CancelFlag::new()))
}

pub fn begin_assistant_turn() -> CancelFlag {
    let flag = CancelFlag::new();
    if let Ok(mut slot) = assistant_cancel_slot().lock() {
        *slot = flag.clone();
    }
    flag
}

pub fn stop_assistant_turn() {
    if let Ok(slot) = assistant_cancel_slot().lock() {
        slot.cancel();
    }
}

pub fn current_assistant_cancel() -> CancelFlag {
    assistant_cancel_slot()
        .lock()
        .map(|slot| slot.clone())
        .unwrap_or_default()
}

pub async fn open_model_manager(
    data_dir: &Path,
    db: &promptlab_storage::Database,
) -> CommandResult<LocalModelManager> {
    LocalModelManager::new_with_db(models_vault_path(data_dir), db.clone())
        .await
        .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))
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
    .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))
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
            harness_factory: promptlab_harness::HarnessFactory::new().unwrap_or_else(|_| {
                promptlab_harness::HarnessFactory::from_registry(Default::default())
            }),
            cancel: current_assistant_cancel(),
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
    let client = session
        .client()
        .await
        .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))?;
    let mut config = JudgeProviderConfig::default();
    config.mode = if entry.provider == ModelProvider::Remote {
        JudgeMode::RemoteLlm
    } else {
        JudgeMode::LocalLlm
    };
    build_judge_engine_with_client(client, &config)
        .await
        .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))
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
            tools: Vec::new(),
            tool_choice: None,
            messages: Vec::new(),
            purpose: None,
        })
        .await
        .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))
}

pub async fn gateway_complete_as(
    data_dir: &Path,
    inference: &InferenceRuntimeManager,
    model_manager: &LocalModelManager,
    model_provider: SharedModelProvider,
    runtime_manager: &mut RuntimeManager,
    agent_id: &str,
    system: Option<&str>,
    prompt: &str,
    max_tokens: u32,
    temperature: f32,
) -> CommandResult<String> {
    let outcome = gateway_complete_outcome_as(
        data_dir,
        inference,
        model_manager,
        model_provider,
        runtime_manager,
        agent_id,
        system,
        prompt,
        max_tokens,
        temperature,
        &[],
        None,
    )
    .await?;
    outcome.content.ok_or_else(|| {
        CommandError::from(PromptLabError::internal(
            "model returned tool_calls without text content",
        ))
    })
}

pub async fn gateway_complete_outcome_as(
    data_dir: &Path,
    inference: &InferenceRuntimeManager,
    model_manager: &LocalModelManager,
    model_provider: SharedModelProvider,
    runtime_manager: &mut RuntimeManager,
    agent_id: &str,
    system: Option<&str>,
    prompt: &str,
    max_tokens: u32,
    temperature: f32,
    tools: &[promptlab_inference::ToolDefinition],
    tool_choice: Option<serde_json::Value>,
) -> CommandResult<promptlab_inference::CompletionOutcome> {
    gateway_complete_outcome_messages_as(
        data_dir,
        inference,
        model_manager,
        model_provider,
        runtime_manager,
        agent_id,
        {
            let mut messages = Vec::new();
            if let Some(system) = system.map(str::trim).filter(|s| !s.is_empty()) {
                messages.push(serde_json::json!({
                    "role": "system",
                    "content": system,
                }));
            }
            messages.push(serde_json::json!({
                "role": "user",
                "content": prompt,
            }));
            messages
        },
        max_tokens,
        temperature,
        tools,
        tool_choice,
    )
    .await
}

/// OpenAI-style multi-turn tool calling (`messages[]` with assistant/tool roles).
pub async fn gateway_complete_outcome_messages_as(
    data_dir: &Path,
    inference: &InferenceRuntimeManager,
    model_manager: &LocalModelManager,
    model_provider: SharedModelProvider,
    runtime_manager: &mut RuntimeManager,
    agent_id: &str,
    messages: Vec<serde_json::Value>,
    max_tokens: u32,
    temperature: f32,
    tools: &[promptlab_inference::ToolDefinition],
    tool_choice: Option<serde_json::Value>,
) -> CommandResult<promptlab_inference::CompletionOutcome> {
    let data_dir = data_dir.to_path_buf();
    let agent_id = agent_id.to_string();
    let tools = tools.to_vec();
    promptlab_inference::with_agent(&agent_id, || async {
        let mut session = open_gateway_session(
            &data_dir,
            inference,
            model_manager,
            model_provider,
            runtime_manager,
        )
        .await?;
        session
            .client()
            .await
            .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))?
            .complete_outcome(CompleteRequest {
                prompt: String::new(),
                system: None,
                max_tokens: Some(max_tokens),
                temperature: Some(temperature),
                tools,
                tool_choice,
                messages,
                purpose: Some(agent_id.clone()),
            })
            .await
            .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))
    })
    .await
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
    async fn complete(&self, prompt: &str) -> promptlab_planner::PlannerResult<String> {
        let inference = self.inference.lock().await;
        let manager = self.model_manager.lock().await;
        let mut runtime_mgr = self.runtime_manager.lock().await;
        gateway_complete_as(
            &self.data_dir,
            &inference,
            &manager,
            self.model_provider.clone(),
            &mut runtime_mgr,
            "attack_plan",
            Some(PromptRegistry::wizard_profile_system()),
            prompt,
            8192,
            0.1,
        )
        .await
        .map_err(|e| promptlab_planner::PlannerError::Llm(e.to_string()))
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
    async fn complete(&self, prompt: &str) -> promptlab_planner::PlannerResult<String> {
        let inference = self.inference.lock().await;
        let manager = self.model_manager.lock().await;
        let mut runtime_mgr = self.runtime_manager.lock().await;
        gateway_complete_as(
            &self.data_dir,
            &inference,
            &manager,
            self.model_provider.clone(),
            &mut runtime_mgr,
            "analyze_endpoint",
            Some(PromptRegistry::endpoint_verify_system()),
            prompt,
            1024,
            0.1,
        )
        .await
        .map_err(|e| promptlab_planner::PlannerError::Llm(e.to_string()))
    }
}

/// Yazg supervisor ReAct LLM — reasons and chooses sub-agent actions.
pub struct HostYazgReactLlm {
    data_dir: PathBuf,
    inference: Arc<AsyncMutex<InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    agent_id: String,
}

impl Clone for HostYazgReactLlm {
    fn clone(&self) -> Self {
        Self {
            data_dir: self.data_dir.clone(),
            inference: self.inference.clone(),
            model_manager: self.model_manager.clone(),
            model_provider: self.model_provider.clone(),
            runtime_manager: self.runtime_manager.clone(),
            agent_id: self.agent_id.clone(),
        }
    }
}

impl HostYazgReactLlm {
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
            agent_id: "yazg".into(),
        }
    }

    pub fn with_agent_id(mut self, agent_id: impl Into<String>) -> Self {
        let id = agent_id.into();
        let trimmed = id.trim();
        self.agent_id = if trimmed.is_empty() {
            "yazg".into()
        } else {
            trimmed.to_string()
        };
        self
    }
}

#[async_trait]
impl PlannerLlm for HostYazgReactLlm {
    async fn complete(&self, prompt: &str) -> promptlab_planner::PlannerResult<String> {
        self.complete_with_system(Some(PromptRegistry::yazg_react_system()), prompt)
            .await
    }

    async fn complete_with_system(
        &self,
        system: Option<&str>,
        prompt: &str,
    ) -> promptlab_planner::PlannerResult<String> {
        let inference = self.inference.lock().await;
        let manager = self.model_manager.lock().await;
        let mut runtime_mgr = self.runtime_manager.lock().await;
        let system = system
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| PromptRegistry::yazg_react_system());
        gateway_complete_as(
            &self.data_dir,
            &inference,
            &manager,
            self.model_provider.clone(),
            &mut runtime_mgr,
            &self.agent_id,
            Some(system),
            prompt,
            1024,
            0.2,
        )
        .await
        .map_err(|e| promptlab_planner::PlannerError::Llm(e.to_string()))
    }

    async fn complete_with_tools(
        &self,
        prompt: &str,
        tools: &[promptlab_planner::ToolSpec],
    ) -> promptlab_planner::PlannerResult<promptlab_planner::LlmCompletion> {
        self.complete_with_tools_and_system(
            Some(PromptRegistry::yazg_react_system()),
            prompt,
            tools,
        )
        .await
    }

    async fn complete_with_tools_and_system(
        &self,
        system: Option<&str>,
        prompt: &str,
        tools: &[promptlab_planner::ToolSpec],
    ) -> promptlab_planner::PlannerResult<promptlab_planner::LlmCompletion> {
        let inference = self.inference.lock().await;
        let manager = self.model_manager.lock().await;
        let mut runtime_mgr = self.runtime_manager.lock().await;
        let wire_tools: Vec<promptlab_inference::ToolDefinition> = tools
            .iter()
            .map(|tool| promptlab_inference::ToolDefinition {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            })
            .collect();
        let system = system
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| PromptRegistry::yazg_react_system());
        let outcome = gateway_complete_outcome_as(
            &self.data_dir,
            &inference,
            &manager,
            self.model_provider.clone(),
            &mut runtime_mgr,
            &self.agent_id,
            Some(system),
            prompt,
            1024,
            0.2,
            &wire_tools,
            Some(serde_json::json!("auto")),
        )
        .await
        .map_err(|e| promptlab_planner::PlannerError::Llm(e.to_string()))?;
        Ok(promptlab_planner::LlmCompletion {
            content: outcome.content,
            tool_calls: outcome
                .tool_calls
                .into_iter()
                .map(|call| promptlab_planner::ToolCall {
                    id: call.id,
                    name: call.name,
                    arguments: call.arguments,
                })
                .collect(),
            input_tokens: outcome.input_tokens,
            output_tokens: outcome.output_tokens,
        })
    }

    async fn complete_with_tools_messages(
        &self,
        messages: &[serde_json::Value],
        tools: &[promptlab_planner::ToolSpec],
    ) -> promptlab_planner::PlannerResult<promptlab_planner::LlmCompletion> {
        let inference = self.inference.lock().await;
        let manager = self.model_manager.lock().await;
        let mut runtime_mgr = self.runtime_manager.lock().await;
        let wire_tools: Vec<promptlab_inference::ToolDefinition> = tools
            .iter()
            .map(|tool| promptlab_inference::ToolDefinition {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            })
            .collect();
        let outcome = gateway_complete_outcome_messages_as(
            &self.data_dir,
            &inference,
            &manager,
            self.model_provider.clone(),
            &mut runtime_mgr,
            &self.agent_id,
            messages.to_vec(),
            1024,
            0.2,
            &wire_tools,
            Some(serde_json::json!("auto")),
        )
        .await
        .map_err(|e| promptlab_planner::PlannerError::Llm(e.to_string()))?;
        Ok(promptlab_planner::LlmCompletion {
            content: outcome.content,
            tool_calls: outcome
                .tool_calls
                .into_iter()
                .map(|call| promptlab_planner::ToolCall {
                    id: call.id,
                    name: call.name,
                    arguments: call.arguments,
                })
                .collect(),
            input_tokens: outcome.input_tokens,
            output_tokens: outcome.output_tokens,
        })
    }
}

/// Attack Factory GeneratePromptAgent LLM — invents novel technique probes.
pub struct HostGeneratePromptLlm {
    data_dir: PathBuf,
    inference: Arc<AsyncMutex<InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
}

impl HostGeneratePromptLlm {
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
impl PlannerLlm for HostGeneratePromptLlm {
    async fn complete(&self, prompt: &str) -> promptlab_planner::PlannerResult<String> {
        let inference = self.inference.lock().await;
        let manager = self.model_manager.lock().await;
        let mut runtime_mgr = self.runtime_manager.lock().await;
        gateway_complete_as(
            &self.data_dir,
            &inference,
            &manager,
            self.model_provider.clone(),
            &mut runtime_mgr,
            "generate_prompt",
            Some(PromptRegistry::attack_catalog_prompt_system()),
            prompt,
            1024,
            0.35,
        )
        .await
        .map_err(|e| promptlab_planner::PlannerError::Llm(e.to_string()))
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
    async fn complete(&self, prompt: &str) -> promptlab_planner::PlannerResult<String> {
        let inference = self.inference.lock().await;
        let manager = self.model_manager.lock().await;
        let mut runtime_mgr = self.runtime_manager.lock().await;
        gateway_complete_as(
            &self.data_dir,
            &inference,
            &manager,
            self.model_provider.clone(),
            &mut runtime_mgr,
            "recommend",
            Some(PromptRegistry::attack_results_recommend_system()),
            prompt,
            2048,
            0.15,
        )
        .await
        .map_err(|e| promptlab_planner::PlannerError::Llm(e.to_string()))
    }
}

/// Per-finding remediation LLM — concrete fix steps for one finding only.
pub struct HostFindingRecommendLlm {
    data_dir: PathBuf,
    inference: Arc<AsyncMutex<InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
}

impl HostFindingRecommendLlm {
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
impl PlannerLlm for HostFindingRecommendLlm {
    async fn complete(&self, prompt: &str) -> promptlab_planner::PlannerResult<String> {
        let inference = self.inference.lock().await;
        let manager = self.model_manager.lock().await;
        let mut runtime_mgr = self.runtime_manager.lock().await;
        gateway_complete_as(
            &self.data_dir,
            &inference,
            &manager,
            self.model_provider.clone(),
            &mut runtime_mgr,
            "recommend",
            Some(PromptRegistry::finding_remediation_recommend_system()),
            prompt,
            3072,
            0.15,
        )
        .await
        .map_err(|e| promptlab_planner::PlannerError::Llm(e.to_string()))
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
    async fn complete(&self, prompt: &str) -> promptlab_planner::PlannerResult<String> {
        let inference = self.inference.lock().await;
        let manager = self.model_manager.lock().await;
        let mut runtime_mgr = self.runtime_manager.lock().await;
        gateway_complete_as(
            &self.data_dir,
            &inference,
            &manager,
            self.model_provider.clone(),
            &mut runtime_mgr,
            "summary",
            Some(PromptRegistry::project_summary_system()),
            prompt,
            2048,
            0.15,
        )
        .await
        .map_err(|e| promptlab_planner::PlannerError::Llm(e.to_string()))
    }
}

/// Scan-level summary LLM — posture overview for a single attack scan.
pub struct HostScanSummaryLlm {
    data_dir: PathBuf,
    inference: Arc<AsyncMutex<InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
}

impl HostScanSummaryLlm {
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
impl PlannerLlm for HostScanSummaryLlm {
    async fn complete(&self, prompt: &str) -> promptlab_planner::PlannerResult<String> {
        let inference = self.inference.lock().await;
        let manager = self.model_manager.lock().await;
        let mut runtime_mgr = self.runtime_manager.lock().await;
        gateway_complete_as(
            &self.data_dir,
            &inference,
            &manager,
            self.model_provider.clone(),
            &mut runtime_mgr,
            "summary",
            Some(PromptRegistry::scan_summary_system()),
            prompt,
            2048,
            0.15,
        )
        .await
        .map_err(|e| promptlab_planner::PlannerError::Llm(e.to_string()))
    }
}

/// Owned host LLMs for a Yazg ReAct turn (project-summary system for the summary slot).
pub struct YazgHostLlms {
    pub supervisor: HostYazgReactLlm,
    pub analyze: HostEndpointVerifyLlm,
    pub plan: HostWizardPlannerLlm,
    pub prompt: HostGeneratePromptLlm,
    pub recommend: HostAttackRecommendLlm,
    pub summary: HostProjectSummaryLlm,
}

impl YazgHostLlms {
    pub fn from_app(
        data_dir: PathBuf,
        inference: Arc<AsyncMutex<InferenceRuntimeManager>>,
        model_manager: Arc<AsyncMutex<LocalModelManager>>,
        model_provider: SharedModelProvider,
        runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    ) -> Self {
        Self {
            supervisor: HostYazgReactLlm::new(
                data_dir.clone(),
                inference.clone(),
                model_manager.clone(),
                model_provider.clone(),
                runtime_manager.clone(),
            ),
            analyze: HostEndpointVerifyLlm::new(
                data_dir.clone(),
                inference.clone(),
                model_manager.clone(),
                model_provider.clone(),
                runtime_manager.clone(),
            ),
            plan: HostWizardPlannerLlm::new(
                data_dir.clone(),
                inference.clone(),
                model_manager.clone(),
                model_provider.clone(),
                runtime_manager.clone(),
            ),
            prompt: HostGeneratePromptLlm::new(
                data_dir.clone(),
                inference.clone(),
                model_manager.clone(),
                model_provider.clone(),
                runtime_manager.clone(),
            ),
            recommend: HostAttackRecommendLlm::new(
                data_dir.clone(),
                inference.clone(),
                model_manager.clone(),
                model_provider.clone(),
                runtime_manager.clone(),
            ),
            summary: HostProjectSummaryLlm::new(
                data_dir,
                inference,
                model_manager,
                model_provider,
                runtime_manager,
            ),
        }
    }

    pub fn into_yazg_llms(self) -> promptlab_agent::YazgLlms {
        let judge = self.supervisor.clone().with_agent_id("judge_coordinator");
        promptlab_agent::YazgLlms {
            supervisor: Arc::new(self.supervisor),
            analyze: Arc::new(self.analyze),
            plan: Arc::new(self.plan),
            prompt: Arc::new(self.prompt),
            recommend: Arc::new(self.recommend),
            summary: Arc::new(self.summary),
            judge: Arc::new(judge),
        }
    }
}

/// Owned host LLMs for a Yazg turn that needs scan-summary system prompts.
pub struct YazgHostLlmsScanSummary {
    pub supervisor: HostYazgReactLlm,
    pub analyze: HostEndpointVerifyLlm,
    pub plan: HostWizardPlannerLlm,
    pub prompt: HostGeneratePromptLlm,
    pub recommend: HostAttackRecommendLlm,
    pub summary: HostScanSummaryLlm,
}

impl YazgHostLlmsScanSummary {
    pub fn from_app(
        data_dir: PathBuf,
        inference: Arc<AsyncMutex<InferenceRuntimeManager>>,
        model_manager: Arc<AsyncMutex<LocalModelManager>>,
        model_provider: SharedModelProvider,
        runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    ) -> Self {
        Self {
            supervisor: HostYazgReactLlm::new(
                data_dir.clone(),
                inference.clone(),
                model_manager.clone(),
                model_provider.clone(),
                runtime_manager.clone(),
            ),
            analyze: HostEndpointVerifyLlm::new(
                data_dir.clone(),
                inference.clone(),
                model_manager.clone(),
                model_provider.clone(),
                runtime_manager.clone(),
            ),
            plan: HostWizardPlannerLlm::new(
                data_dir.clone(),
                inference.clone(),
                model_manager.clone(),
                model_provider.clone(),
                runtime_manager.clone(),
            ),
            prompt: HostGeneratePromptLlm::new(
                data_dir.clone(),
                inference.clone(),
                model_manager.clone(),
                model_provider.clone(),
                runtime_manager.clone(),
            ),
            recommend: HostAttackRecommendLlm::new(
                data_dir.clone(),
                inference.clone(),
                model_manager.clone(),
                model_provider.clone(),
                runtime_manager.clone(),
            ),
            summary: HostScanSummaryLlm::new(
                data_dir,
                inference,
                model_manager,
                model_provider,
                runtime_manager,
            ),
        }
    }

    pub fn into_yazg_llms(self) -> promptlab_agent::YazgLlms {
        let judge = self.supervisor.clone().with_agent_id("judge_coordinator");
        promptlab_agent::YazgLlms {
            supervisor: Arc::new(self.supervisor),
            analyze: Arc::new(self.analyze),
            plan: Arc::new(self.plan),
            prompt: Arc::new(self.prompt),
            recommend: Arc::new(self.recommend),
            summary: Arc::new(self.summary),
            judge: Arc::new(judge),
        }
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
    async fn complete(&self, prompt: &str) -> promptlab_generator::GeneratorResult<String> {
        let inference = self.inference.lock().await;
        let manager = self.model_manager.lock().await;
        let mut runtime_mgr = self.runtime_manager.lock().await;
        gateway_complete_as(
            &self.data_dir,
            &inference,
            &manager,
            self.model_provider.clone(),
            &mut runtime_mgr,
            "generate_prompt",
            Some(PromptRegistry::generator_system()),
            prompt,
            1536,
            0.2,
        )
        .await
        .map_err(|e| promptlab_generator::GeneratorError::Llm(e.to_string()))
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
            config.mode = InferenceMode::ThirdParty;
            config.provider = InferenceProvider::Ollama;
            config.runtime = "ollama".into();
        }
        _ => {
            return Err(CommandError::invalid_input(
                "use a remote provider or Ollama over HTTP",
            ));
        }
    }
    Ok(())
}

pub async fn test_remote_connectivity_only(
    entry: &ModelEntry,
    remote: RemoteAdapterSettings,
) -> CommandResult<ConnectivityTestResult> {
    use promptlab_inference::{InferenceClient, RemoteProviderAdapter};
    use std::sync::Arc;

    let client = InferenceClient::from_adapter(
        Arc::new(RemoteProviderAdapter::new(remote.clone())),
        32,
        0.0,
    );
    let started = std::time::Instant::now();
    let latency_ms = || started.elapsed().as_millis() as u64;

    let result = client.health().await;

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
            harness_factory: promptlab_harness::HarnessFactory::new().unwrap_or_else(|_| {
                promptlab_harness::HarnessFactory::from_registry(Default::default())
            }),
            cancel: current_assistant_cancel(),
        },
    };
    session
        .test_connectivity()
        .await
        .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))
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
            harness_factory: promptlab_harness::HarnessFactory::new().unwrap_or_else(|_| {
                promptlab_harness::HarnessFactory::from_registry(Default::default())
            }),
            cancel: current_assistant_cancel(),
        },
    };
    session
        .test_inference()
        .await
        .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))
}

pub fn connectivity_to_judge(result: ConnectivityTestResult) -> promptlab_judge::JudgeConnectivityResult {
    promptlab_judge::JudgeConnectivityResult {
        ok: result.ok,
        provider: result.provider,
        model: result.model,
        latency_ms: result.latency_ms,
        message: result.message,
        sample_response: result.sample_response,
    }
}
