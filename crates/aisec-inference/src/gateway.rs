use std::sync::Arc;

use aisec_models::ModelEntry;
use aisec_runtime::{RuntimeManager, SharedModelProvider};
use async_trait::async_trait;

use crate::capabilities::ModelCapabilities;
use crate::config::InferenceMode;
use crate::error::{InferenceError, InferenceResult};
use crate::manager::InferenceRuntimeManager;
use crate::prompts::PromptComposer;
use crate::provider::{ProviderAdapter, RemoteAdapterSettings};
use crate::types::{
    ChatRequest, ChatResponse, CompleteRequest, ConnectivityTestResult, EmbedRequest,
    EmbedResponse, HealthStatus, JsonGenerateRequest, StreamChunk, StreamRequest,
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
    async fn chat(&self, request: ChatRequest) -> InferenceResult<ChatResponse> {
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

/// Session-scoped gateway operations (used by desktop shell).
pub struct GatewaySession<'a> {
    pub inner: InferenceSession<'a>,
}

impl<'a> GatewaySession<'a> {
    pub async fn complete(&mut self, request: CompleteRequest) -> InferenceResult<String> {
        let adapter = DefaultAiInferenceGateway::adapter_for(&mut self.inner).await?;
        let config = self.inner.manager.config();
        let max_tokens = request.max_tokens.unwrap_or(config.max_tokens);
        let temperature = request.temperature.unwrap_or(config.temperature);
        adapter
            .complete(request.system.as_deref(), &request.prompt, max_tokens, temperature)
            .await
    }

    pub async fn chat(&mut self, request: ChatRequest) -> InferenceResult<ChatResponse> {
        let adapter = DefaultAiInferenceGateway::adapter_for(&mut self.inner).await?;
        let config = self.inner.manager.config();
        let max_tokens = request.max_tokens.unwrap_or(config.max_tokens);
        let temperature = request.temperature.unwrap_or(config.temperature);
        let content = adapter.chat(&request.messages, max_tokens, temperature).await?;
        Ok(ChatResponse {
            content,
            model: adapter.model_id().into(),
            provider: adapter.provider_id().into(),
        })
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
            })
            .await?;
        extract_json_value(&raw)
    }

    pub async fn health(&mut self) -> InferenceResult<HealthStatus> {
        let adapter = DefaultAiInferenceGateway::adapter_for(&mut self.inner).await?;
        self.inner.manager.health_check(adapter.as_ref()).await
    }

    pub async fn capabilities(&mut self) -> InferenceResult<ModelCapabilities> {
        if self.inner.manager.config().mode == InferenceMode::Deterministic {
            return Ok(ModelCapabilities::deterministic());
        }
        let adapter = DefaultAiInferenceGateway::adapter_for(&mut self.inner).await?;
        Ok(self.inner.manager.capabilities_for(adapter.as_ref()))
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
            })
            .await
    }
}
