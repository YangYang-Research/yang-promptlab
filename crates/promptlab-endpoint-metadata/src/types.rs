use std::collections::HashMap;

use promptlab_fingerprint::StackFingerprintReport;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Complete AI-aware endpoint metadata — persisted once during Discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiEndpointMetadata {
    pub basic: EndpointBasic,
    pub fingerprint: FingerprintMetadata,
    pub schema: SchemaMetadata,
    pub inference: InferenceFields,
    pub capabilities: EndpointCapabilities,
    pub classification: EndpointClassification,
    pub risk: RiskAssessment,
    pub provenance: DiscoveryProvenance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<RawObservation>,
    /// Full stack fingerprint report embedded for planner attack recommendations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack_fingerprint: Option<StackFingerprintReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointBasic {
    pub id: String,
    pub url: String,
    pub method: String,
    pub host: String,
    pub protocol: String,
    pub status: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintMetadata {
    pub framework: String,
    pub provider: String,
    pub version: String,
    pub confidence: f32,
    pub api_style: String,
    pub technologies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SchemaMetadata {
    pub content_type: Option<String>,
    pub request_schema: Option<NormalizedSchema>,
    pub response_schema: Option<NormalizedSchema>,
    pub transport: Vec<TransportKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    Json,
    Multipart,
    Graphql,
    Mcp,
    Websocket,
    Sse,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedSchema {
    pub format: String,
    pub fields: Vec<SchemaField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaField {
    pub name: String,
    pub field_type: String,
    pub required: bool,
}

/// Inferred request field paths for payload mutation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InferenceFields {
    pub prompt_field: Option<String>,
    pub history_field: Option<String>,
    pub conversation_field: Option<String>,
    pub model_field: Option<String>,
    pub stream_field: Option<String>,
    pub tool_field: Option<String>,
    pub attachment_field: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EndpointCapabilities {
    pub supports_chat: bool,
    pub supports_streaming: bool,
    pub supports_embedding: bool,
    pub supports_vision: bool,
    pub supports_tools: bool,
    pub supports_json_mode: bool,
    pub supports_thinking: bool,
    pub supports_memory: bool,
    pub supports_agent: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EndpointType {
    AiChat,
    AiAgent,
    Embedding,
    Completion,
    ImageGeneration,
    Speech,
    Moderation,
    Workflow,
    ToolEndpoint,
    Mcp,
    UnknownAi,
    NonAi,
}

impl EndpointType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AiChat => "ai_chat",
            Self::AiAgent => "ai_agent",
            Self::Embedding => "embedding",
            Self::Completion => "completion",
            Self::ImageGeneration => "image_generation",
            Self::Speech => "speech",
            Self::Moderation => "moderation",
            Self::Workflow => "workflow",
            Self::ToolEndpoint => "tool_endpoint",
            Self::Mcp => "mcp",
            Self::UnknownAi => "unknown_ai",
            Self::NonAi => "non_ai",
        }
    }

    pub fn from_str_lossy(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "ai_chat" => Self::AiChat,
            "ai_agent" => Self::AiAgent,
            "embedding" => Self::Embedding,
            "completion" => Self::Completion,
            "image_generation" => Self::ImageGeneration,
            "speech" => Self::Speech,
            "moderation" => Self::Moderation,
            "workflow" => Self::Workflow,
            "tool_endpoint" => Self::ToolEndpoint,
            "mcp" => Self::Mcp,
            "non_ai" => Self::NonAi,
            _ => Self::UnknownAi,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointClassification {
    pub endpoint_type: EndpointType,
    pub ai_framework: String,
    pub confidence: f32,
    pub risk_score: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskAssessment {
    pub score: u8,
    pub factors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryProvenance {
    pub discovery_source: String,
    pub authentication_required: bool,
    pub discovered_at: OffsetDateTime,
    pub kind: String,
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawObservation {
    pub request_headers: HashMap<String, String>,
    pub request_body: Option<String>,
    pub response_headers: HashMap<String, String>,
    pub response_body: Option<String>,
}

impl AiEndpointMetadata {
    pub fn to_json(&self) -> promptlab_core::PromptLabResult<String> {
        serde_json::to_string(self).map_err(|e| promptlab_core::PromptLabError::internal(e.to_string()))
    }

    pub fn from_json(raw: &str) -> promptlab_core::PromptLabResult<Self> {
        serde_json::from_str(raw).map_err(|e| promptlab_core::PromptLabError::internal(e.to_string()))
    }

    pub fn endpoint_type_str(&self) -> &str {
        self.classification.endpoint_type.as_str()
    }

    pub fn risk_score(&self) -> u8 {
        self.risk.score
    }

    pub fn framework(&self) -> &str {
        &self.fingerprint.framework
    }
}
