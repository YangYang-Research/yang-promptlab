use std::sync::Arc;

use promptlab_harness::{
    AttackRequest, CancelFlag, ChatMessage, ChatTool, HarnessFactory, HarnessPurpose,
    TargetDescriptor, TargetSurface,
};
use promptlab_models::ModelEntry;
use promptlab_runtime::{RuntimeManager, SharedModelProvider};
use async_trait::async_trait;

use crate::capabilities::ModelCapabilities;
use crate::config::InferenceMode;
use crate::error::{InferenceError, InferenceResult};
use crate::manager::InferenceRuntimeManager;
use crate::prompts::PromptComposer;
use crate::provider::{
    descriptor_from_remote, AdapterHarness, ProviderAdapter, RemoteAdapterSettings,
};
use crate::types::{
    ChatRequest, ChatResponse, CompleteRequest, CompletionOutcome, ConnectivityTestResult,
    EmbedRequest, EmbedResponse, HealthStatus, JsonGenerateRequest, StreamChunk, StreamRequest,
};

/// Single public inference API — all AI features must use this gateway.
#[async_trait]
pub trait AiInferenceGateway: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> InferenceResult<ChatResponse>;
    async fn complete(&self, request: CompleteRequest) -> InferenceResult<String>;
    async fn generate_json(&self, request: JsonGenerateRequest) -> InferenceResult<serde_json::Value>;
    async fn embed(&self, request: EmbedRequest) -> InferenceResult<EmbedResponse>;
    async fn stream(&self, request: StreamRequest) -> InferenceResult<Vec<StreamChunk>>;
    async fn health(&self) -> InferenceResult<HealthStatus>;
    async fn capabilities(&self) -> InferenceResult<ModelCapabilities>;
    async fn test_connectivity(&self) -> InferenceResult<ConnectivityTestResult>;
    async fn test_inference(&self) -> InferenceResult<ConnectivityTestResult>;
}

pub struct InferenceSession<'a> {
    pub manager: &'a InferenceRuntimeManager,
    pub runtime_manager: &'a mut RuntimeManager,
    pub model_provider: SharedModelProvider,
    pub model_entry: &'a ModelEntry,
    pub remote_settings: Option<RemoteAdapterSettings>,
    pub harness_factory: HarnessFactory,
    pub cancel: promptlab_harness::CancelFlag,
}

pub struct DefaultAiInferenceGateway;

impl DefaultAiInferenceGateway {
    pub async fn adapter_for<'a>(
        session: &mut InferenceSession<'a>,
    ) -> InferenceResult<Arc<dyn ProviderAdapter>> {
        if !session.manager.is_ready() {
            return Err(InferenceError::NotReady(
                "AI runtime is not configured".into(),
            ));
        }
        session
            .manager
            .build_provider_adapter(
                session.model_entry,
                session.remote_settings.clone(),
                session.model_provider.clone(),
                session.runtime_manager,
            )
            .await
    }
}

#[async_trait]
impl AiInferenceGateway for DefaultAiInferenceGateway {
    async fn chat(&self, _request: ChatRequest) -> InferenceResult<ChatResponse> {
        Err(InferenceError::Unsupported(
            "chat requires InferenceSession — use gateway_session()".into(),
        ))
    }

    async fn complete(&self, _request: CompleteRequest) -> InferenceResult<String> {
        Err(InferenceError::Unsupported(
            "complete requires InferenceSession — use complete_with_session()".into(),
        ))
    }

    async fn generate_json(&self, _request: JsonGenerateRequest) -> InferenceResult<serde_json::Value> {
        Err(InferenceError::Unsupported(
            "generate_json requires InferenceSession".into(),
        ))
    }

    async fn embed(&self, _request: EmbedRequest) -> InferenceResult<EmbedResponse> {
        Err(InferenceError::Unsupported("embedding not configured".into()))
    }

    async fn stream(&self, _request: StreamRequest) -> InferenceResult<Vec<StreamChunk>> {
        Err(InferenceError::Unsupported("streaming not yet enabled".into()))
    }

    async fn health(&self) -> InferenceResult<HealthStatus> {
        Err(InferenceError::Unsupported("health requires InferenceSession".into()))
    }

    async fn capabilities(&self) -> InferenceResult<ModelCapabilities> {
        Ok(ModelCapabilities::default())
    }

    async fn test_connectivity(&self) -> InferenceResult<ConnectivityTestResult> {
        Err(InferenceError::Unsupported(
            "test_connectivity requires InferenceSession".into(),
        ))
    }

    async fn test_inference(&self) -> InferenceResult<ConnectivityTestResult> {
        Err(InferenceError::Unsupported(
            "test_inference requires InferenceSession".into(),
        ))
    }
}

/// Cloneable gateway completion handle — hot-path API for agents and judge.
///
/// Resolve once via [`GatewaySession::client`], then share across concurrent tasks.
/// Every completion goes through [`HarnessFactory::execute`].
#[derive(Clone)]
pub struct InferenceClient {
    factory: HarnessFactory,
    descriptor: TargetDescriptor,
    purpose: HarnessPurpose,
    provider_id: String,
    model_id: String,
    capabilities: ModelCapabilities,
    default_max_tokens: u32,
    default_temperature: f32,
    timeout_ms: u64,
    cancel: CancelFlag,
}

impl InferenceClient {
    pub fn from_harness(
        factory: HarnessFactory,
        descriptor: TargetDescriptor,
        purpose: HarnessPurpose,
        provider_id: impl Into<String>,
        model_id: impl Into<String>,
        capabilities: ModelCapabilities,
        default_max_tokens: u32,
        default_temperature: f32,
        timeout_ms: u64,
        cancel: CancelFlag,
    ) -> Self {
        Self {
            factory,
            descriptor,
            purpose,
            provider_id: provider_id.into(),
            model_id: model_id.into(),
            capabilities,
            default_max_tokens,
            default_temperature,
            timeout_ms,
            cancel,
        }
    }

    /// Wrap an already-resolved provider adapter (tests / host bootstrap).
    /// Registers the adapter on an isolated factory so I/O still hits `execute()`.
    pub fn from_adapter(
        adapter: Arc<dyn ProviderAdapter>,
        default_max_tokens: u32,
        default_temperature: f32,
    ) -> Self {
        let factory = HarnessFactory::from_registry(promptlab_harness::HarnessRegistry::new());
        let provider_id = adapter.provider_id().to_string();
        let model_id = adapter.model_id().to_string();
        let capabilities = adapter.capabilities();
        let _ = factory.register(Arc::new(AdapterHarness::new(adapter)));
        Self::from_harness(
            factory,
            TargetDescriptor {
                url: "local://adapter".into(),
                surface: TargetSurface::LocalRuntime,
                ..TargetDescriptor::default()
            },
            HarnessPurpose::assistant(),
            provider_id,
            model_id,
            capabilities,
            default_max_tokens,
            default_temperature,
            120_000,
            CancelFlag::new(),
        )
    }

    pub fn with_purpose(mut self, purpose: HarnessPurpose) -> Self {
        self.purpose = purpose;
        self
    }

    pub fn with_cancel(mut self, cancel: CancelFlag) -> Self {
        self.cancel = cancel;
        self
    }

    pub fn cancel_flag(&self) -> &CancelFlag {
        &self.cancel
    }

    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    pub fn capabilities(&self) -> ModelCapabilities {
        self.capabilities.clone()
    }

    pub async fn complete(&self, request: CompleteRequest) -> InferenceResult<String> {
        let outcome = self.complete_outcome(request).await?;
        outcome.content.ok_or_else(|| {
            InferenceError::Provider("model returned tool_calls without text content".into())
        })
    }

    /// LangChain-style completion that may return native `tool_calls`.
    pub async fn complete_outcome(
        &self,
        request: CompleteRequest,
    ) -> InferenceResult<CompletionOutcome> {
        let max_tokens = request.max_tokens.unwrap_or(self.default_max_tokens);
        let temperature = request.temperature.unwrap_or(self.default_temperature);
        let purpose = request
            .purpose
            .as_deref()
            .map(HarnessPurpose::from_agent_id)
            .unwrap_or_else(|| self.purpose.clone());
        let messages = if !request.messages.is_empty() {
            request
                .messages
                .iter()
                .map(ChatMessage::from_json)
                .collect()
        } else {
            let mut messages = Vec::new();
            if let Some(system) = request
                .system
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                messages.push(ChatMessage::text("system", system));
            }
            messages.push(ChatMessage::text("user", request.prompt));
            messages
        };
        let mut harness_request = AttackRequest::from_chat(self.descriptor.url.clone(), messages);
        harness_request.purpose = purpose;
        harness_request.auth = self.descriptor.auth.clone();
        harness_request.headers = self.descriptor.headers.clone();
        harness_request.model = Some(self.model_id.clone());
        harness_request.max_tokens = Some(max_tokens);
        harness_request.temperature = Some(temperature);
        harness_request.timeout_ms = self.timeout_ms;
        harness_request.cancel = self.cancel.clone();
        harness_request.tools = request
            .tools
            .iter()
            .map(|tool| ChatTool {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            })
            .collect();
        harness_request.tool_choice = request.tool_choice.clone();
        crate::traffic::record_sent();
        let response = self
            .factory
            .execute(&self.descriptor, harness_request)
            .await?;
        crate::traffic::record_received();
        let input = response.usage_input_tokens.unwrap_or(0);
        let output = response.usage_output_tokens.unwrap_or(0);
        if input > 0 || output > 0 {
            crate::token_usage::record_completion(input, output);
        }
        if response.content.trim().is_empty() && response.tool_calls.is_empty() {
            return Err(InferenceError::Provider(
                "model returned empty content".into(),
            ));
        }
        Ok(CompletionOutcome {
            content: if response.content.trim().is_empty() {
                None
            } else {
                Some(response.content)
            },
            tool_calls: response
                .tool_calls
                .into_iter()
                .map(|call| crate::types::ToolCall {
                    id: call.id.unwrap_or_else(|| format!("call_{}", call.name)),
                    name: call.name,
                    arguments: serde_json::from_str(&call.arguments)
                        .unwrap_or_else(|_| serde_json::json!({})),
                })
                .collect(),
            input_tokens: input,
            output_tokens: output,
        })
    }

    pub async fn chat(&self, request: ChatRequest) -> InferenceResult<ChatResponse> {
        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .map(|message| {
                serde_json::json!({"role": message.role, "content": message.content})
            })
            .collect();
        let content = self
            .complete(CompleteRequest {
                prompt: String::new(),
                system: None,
                max_tokens: request.max_tokens,
                temperature: request.temperature,
                tools: Vec::new(),
                tool_choice: None,
                messages,
                purpose: Some(self.purpose.as_str().to_string()),
            })
            .await?;
        Ok(ChatResponse {
            content,
            model: self.model_id.clone(),
            provider: self.provider_id.clone(),
        })
    }

    /// Provider health probe through the same factory path.
    pub async fn health(&self) -> InferenceResult<bool> {
        let sample = self
            .complete(CompleteRequest {
                prompt: crate::prompts::PromptRegistry::health_check_user().into(),
                system: Some(crate::prompts::PromptRegistry::health_check_system().into()),
                max_tokens: Some(32),
                temperature: Some(0.0),
                tools: Vec::new(),
                tool_choice: None,
                messages: Vec::new(),
                purpose: Some(HarnessPurpose::health().as_str().to_string()),
            })
            .await;
        match sample {
            Ok(text) => Ok(!text.trim().is_empty()),
            Err(err) => Err(err),
        }
    }
}

/// Session-scoped gateway operations (used by desktop shell).
pub struct GatewaySession<'a> {
    pub inner: InferenceSession<'a>,
}

impl<'a> GatewaySession<'a> {
    /// Resolve the active provider once into a cloneable [`InferenceClient`].
    pub async fn client(&mut self) -> InferenceResult<InferenceClient> {
        if !self.inner.manager.is_ready() {
            return Err(InferenceError::NotReady(
                "AI runtime is not configured".into(),
            ));
        }
        let config = self.inner.manager.config().clone();
        let factory = self.inner.harness_factory.isolated();
        let cancel = self.inner.cancel.clone();
        match config.mode {
            InferenceMode::Deterministic => Err(InferenceError::NotReady(
                "deterministic mode has no LLM provider".into(),
            )),
            InferenceMode::ThirdParty => {
                let settings = self.inner.remote_settings.clone().ok_or_else(|| {
                    InferenceError::Config("missing remote credentials".into())
                })?;
                let descriptor = descriptor_from_remote(&settings);
                Ok(InferenceClient::from_harness(
                    factory,
                    descriptor,
                    HarnessPurpose::assistant(),
                    settings.provider.as_str(),
                    settings.model,
                    crate::capabilities::ModelCapabilities::from_remote(settings.provider.as_str()),
                    config.max_tokens,
                    config.temperature,
                    config.timeout_secs.saturating_mul(1000).max(1_000),
                    cancel,
                ))
            }
        }
    }

    pub async fn complete(&mut self, request: CompleteRequest) -> InferenceResult<String> {
        self.client().await?.complete(request).await
    }

    pub async fn chat(&mut self, request: ChatRequest) -> InferenceResult<ChatResponse> {
        self.client().await?.chat(request).await
    }

    pub async fn generate_json(
        &mut self,
        request: JsonGenerateRequest,
    ) -> InferenceResult<serde_json::Value> {
        let prompt = PromptComposer::compose(request.system.as_deref(), &request.prompt);
        let raw = self
            .complete(CompleteRequest {
                prompt,
                system: None,
                max_tokens: request.max_tokens,
                temperature: request.temperature,
                tools: Vec::new(),
                tool_choice: None,
                messages: Vec::new(),
                purpose: None,
            })
            .await?;
        extract_json_value(&raw)
    }

    pub async fn health(&mut self) -> InferenceResult<HealthStatus> {
        let started = std::time::Instant::now();
        let client = self.client().await?;
        let ok = client.health().await.unwrap_or(false);
        Ok(HealthStatus {
            ok,
            provider: client.provider_id().into(),
            model: client.model_id().into(),
            message: if ok {
                "healthy".into()
            } else {
                "unhealthy".into()
            },
            latency_ms: started.elapsed().as_millis() as u64,
        })
    }

    pub async fn capabilities(&mut self) -> InferenceResult<ModelCapabilities> {
        if self.inner.manager.config().mode == InferenceMode::Deterministic {
            return Ok(ModelCapabilities::deterministic());
        }
        Ok(self.client().await?.capabilities())
    }

    pub async fn test_connectivity(&mut self) -> InferenceResult<ConnectivityTestResult> {
        let started = std::time::Instant::now();
        let health = self.health().await?;
        Ok(ConnectivityTestResult {
            ok: health.ok,
            provider: health.provider,
            model: health.model,
            latency_ms: started.elapsed().as_millis() as u64,
            message: health.message,
            sample_response: None,
        })
    }

    pub async fn test_inference(&mut self) -> InferenceResult<ConnectivityTestResult> {
        let started = std::time::Instant::now();
        let sample = self
            .complete(CompleteRequest {
                prompt: crate::prompts::PromptRegistry::health_check_user().into(),
                system: Some(crate::prompts::PromptRegistry::health_check_system().into()),
                max_tokens: Some(32),
                temperature: Some(0.0),
                tools: Vec::new(),
                tool_choice: None,
                messages: Vec::new(),
                purpose: None,
            })
            .await?;
        Ok(ConnectivityTestResult {
            ok: !sample.trim().is_empty(),
            provider: self.inner.manager.config().provider.as_str().into(),
            model: self.inner.manager.config().model.clone(),
            latency_ms: started.elapsed().as_millis() as u64,
            message: "Inference smoke test succeeded".into(),
            sample_response: Some(sample),
        })
    }

    pub async fn embed(&mut self, _request: EmbedRequest) -> InferenceResult<EmbedResponse> {
        let caps = self.capabilities().await?;
        if !caps.supports_embedding {
            return Err(InferenceError::Unsupported(
                "active model does not support embeddings".into(),
            ));
        }
        Err(InferenceError::Unsupported("embedding route not wired".into()))
    }

    pub async fn stream(&mut self, request: StreamRequest) -> InferenceResult<Vec<StreamChunk>> {
        let caps = self.capabilities().await?;
        if !caps.supports_streaming {
            return Err(InferenceError::Unsupported(
                "active model does not support streaming".into(),
            ));
        }
        let text = self
            .complete(CompleteRequest {
                prompt: request.prompt,
                system: request.system,
                max_tokens: request.max_tokens,
                temperature: request.temperature,
                tools: Vec::new(),
                tool_choice: None,
                messages: Vec::new(),
                purpose: None,
            })
            .await?;
        Ok(vec![StreamChunk {
            delta: text,
            done: true,
        }])
    }
}

fn extract_json_value(raw: &str) -> InferenceResult<serde_json::Value> {
    let trimmed = raw.trim();
    if let Ok(value) = serde_json::from_str(trimmed) {
        return Ok(value);
    }
    let start = trimmed
        .find('{')
        .or_else(|| trimmed.find('['))
        .ok_or_else(|| InferenceError::Serialization("no JSON in response".into()))?;
    let end = trimmed
        .rfind('}')
        .or_else(|| trimmed.rfind(']'))
        .ok_or_else(|| InferenceError::Serialization("unterminated JSON".into()))?;
    serde_json::from_str(&trimmed[start..=end])
        .map_err(|e| InferenceError::Serialization(e.to_string()))
}

/// Bridge for feature crates that need a simple complete() trait.
pub struct GatewayLlmBridge<'a> {
    session: GatewaySession<'a>,
    max_tokens: u32,
    temperature: f32,
}

impl<'a> GatewayLlmBridge<'a> {
    pub fn new(session: GatewaySession<'a>, max_tokens: u32, temperature: f32) -> Self {
        Self {
            session,
            max_tokens,
            temperature,
        }
    }

    pub async fn complete(&mut self, prompt: &str) -> InferenceResult<String> {
        self.session
            .complete(CompleteRequest {
                prompt: prompt.to_string(),
                system: None,
                max_tokens: Some(self.max_tokens),
                temperature: Some(self.temperature),
                tools: Vec::new(),
                tool_choice: None,
                messages: Vec::new(),
                purpose: None,
            })
            .await
    }
}
