use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Known AI inference provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiProvider {
    OpenAi,
    Anthropic,
    Gemini,
    Bedrock,
    AzureOpenAi,
    Ollama,
    LiteLlm,
    Vllm,
    OpenRouter,
}

impl AiProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
            Self::Bedrock => "bedrock",
            Self::AzureOpenAi => "azure_openai",
            Self::Ollama => "ollama",
            Self::LiteLlm => "litellm",
            Self::Vllm => "vllm",
            Self::OpenRouter => "openrouter",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Gemini => "Google Gemini",
            Self::Bedrock => "AWS Bedrock",
            Self::AzureOpenAi => "Azure OpenAI",
            Self::Ollama => "Ollama",
            Self::LiteLlm => "LiteLLM",
            Self::Vllm => "vLLM",
            Self::OpenRouter => "OpenRouter",
        }
    }

    pub fn all() -> &'static [AiProvider] {
        use AiProvider::*;
        &[
            OpenAi,
            Anthropic,
            Gemini,
            Bedrock,
            AzureOpenAi,
            Ollama,
            LiteLlm,
            Vllm,
            OpenRouter,
        ]
    }
}

/// Agent orchestration / UI framework detected on the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentFramework {
    LangChain,
    LangGraph,
    LangServe,
    OpenWebUi,
    AnythingLlm,
    Flowise,
    Dify,
    Langflow,
    LibreChat,
    CrewAi,
    AutoGen,
}

impl AgentFramework {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LangChain => "langchain",
            Self::LangGraph => "langgraph",
            Self::LangServe => "langserve",
            Self::OpenWebUi => "openwebui",
            Self::AnythingLlm => "anythingllm",
            Self::Flowise => "flowise",
            Self::Dify => "dify",
            Self::Langflow => "langflow",
            Self::LibreChat => "librechat",
            Self::CrewAi => "crewai",
            Self::AutoGen => "autogen",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::LangChain => "LangChain",
            Self::LangGraph => "LangGraph",
            Self::LangServe => "LangServe",
            Self::OpenWebUi => "OpenWebUI",
            Self::AnythingLlm => "AnythingLLM",
            Self::Flowise => "Flowise",
            Self::Dify => "Dify",
            Self::Langflow => "Langflow",
            Self::LibreChat => "LibreChat",
            Self::CrewAi => "CrewAI",
            Self::AutoGen => "AutoGen",
        }
    }

    pub fn all() -> &'static [AgentFramework] {
        use AgentFramework::*;
        &[
            LangChain,
            LangGraph,
            LangServe,
            OpenWebUi,
            AnythingLlm,
            Flowise,
            Dify,
            Langflow,
            LibreChat,
            CrewAi,
            AutoGen,
        ]
    }
}

/// AI deployment component (MCP, RAG, tools).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiComponent {
    McpServer,
    RagPipeline,
    ToolOrchestration,
}

impl AiComponent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::McpServer => "mcp_server",
            Self::RagPipeline => "rag_pipeline",
            Self::ToolOrchestration => "tool_orchestration",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::McpServer => "MCP Server",
            Self::RagPipeline => "RAG Pipeline",
            Self::ToolOrchestration => "Tool Orchestration",
        }
    }
}

/// Fingerprint signal source method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FingerprintMethod {
    Headers,
    Responses,
    OpenApi,
    GraphQl,
    JavaScript,
    KnownRoutes,
    Metadata,
}

impl FingerprintMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Headers => "headers",
            Self::Responses => "responses",
            Self::OpenApi => "openapi",
            Self::GraphQl => "graphql",
            Self::JavaScript => "javascript",
            Self::KnownRoutes => "known_routes",
            Self::Metadata => "metadata",
        }
    }
}

/// HTTP observation used for fingerprinting.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FingerprintInput {
    pub url: String,
    pub method: Option<String>,
    pub status: Option<u16>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub kind_hint: Option<String>,
}

impl FingerprintInput {
    pub fn from_parts(
        url: impl Into<String>,
        status: Option<u16>,
        headers: HashMap<String, String>,
        body: Option<String>,
    ) -> Self {
        Self {
            url: url.into(),
            method: None,
            status,
            headers,
            body,
            content_type: None,
            kind_hint: None,
        }
    }

    pub fn from_snapshot(
        url: impl Into<String>,
        method: Option<String>,
        status: u16,
        headers: HashMap<String, String>,
        content_type: Option<String>,
        body: String,
        kind_hint: Option<String>,
    ) -> Self {
        Self {
            url: url.into(),
            method,
            status: Some(status),
            headers,
            body: Some(body),
            content_type,
            kind_hint,
        }
    }
}

/// A matched detection signal contributing to confidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchedSignal {
    pub provider: AiProvider,
    pub rule_id: String,
    pub description: String,
    pub weight: f32,
}

/// Matched stack signal for frameworks/components.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StackSignal {
    pub rule_id: String,
    pub description: String,
    pub weight: f32,
    pub method: FingerprintMethod,
}

/// Detected technology with confidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectedTechnology {
    pub id: String,
    pub name: String,
    pub category: String,
    pub confidence: f32,
    pub signals: Vec<String>,
}

/// Detected agent framework.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectedFramework {
    pub framework: AgentFramework,
    pub name: String,
    pub confidence: f32,
    pub signals: Vec<String>,
}

/// Detected AI component.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectedComponent {
    pub component: AiComponent,
    pub name: String,
    pub confidence: f32,
    pub signals: Vec<String>,
}

/// Attack category recommendation derived from fingerprint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttackRecommendation {
    pub category: String,
    pub reason: String,
    pub priority: u8,
}

/// Provider fingerprint result with confidence score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderFingerprint {
    pub provider: AiProvider,
    pub confidence: f32,
    pub signals: Vec<MatchedSignal>,
    pub inferred_api_style: ApiStyle,
    pub suggested_method: Option<String>,
}

/// API compatibility style inferred from fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiStyle {
    OpenAiCompatible,
    AnthropicMessages,
    GeminiGenerateContent,
    BedrockInvoke,
    OllamaNative,
    Unknown,
}

/// Aggregated fingerprint report for an endpoint (providers only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FingerprintReport {
    pub url: String,
    pub matches: Vec<ProviderFingerprint>,
    pub primary: Option<ProviderFingerprint>,
    pub analyzed_at: OffsetDateTime,
}

impl FingerprintReport {
    pub fn best_match(&self) -> Option<&ProviderFingerprint> {
        self.primary.as_ref().or_else(|| self.matches.first())
    }
}

/// Normalized platform profile for attack planning (pre-attack identification).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PlatformProfile {
    pub platform: String,
    pub version: String,
    pub auth_type: String,
    pub llm_provider: String,
    pub memory_enabled: bool,
    pub tools_enabled: bool,
    pub rag_enabled: bool,
}

/// Full AI stack fingerprint including providers, frameworks, components, and attack plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StackFingerprintReport {
    pub url: String,
    pub confidence: f32,
    pub technologies: Vec<DetectedTechnology>,
    pub agent_frameworks: Vec<DetectedFramework>,
    pub ai_components: Vec<DetectedComponent>,
    pub provider_report: FingerprintReport,
    pub attack_recommendations: Vec<AttackRecommendation>,
    pub methods_used: Vec<String>,
    #[serde(default)]
    pub platform_profile: PlatformProfile,
    pub analyzed_at: OffsetDateTime,
}

impl StackFingerprintReport {
    pub fn primary_technology(&self) -> Option<&DetectedTechnology> {
        self.technologies.first()
    }
}

/// Minimum confidence to include a provider in results.
pub const DEFAULT_CONFIDENCE_THRESHOLD: f32 = 0.45;

pub const FRAMEWORK_CONFIDENCE_THRESHOLD: f32 = 0.40;

pub const COMPONENT_CONFIDENCE_THRESHOLD: f32 = 0.40;
