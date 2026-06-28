use crate::types::{EndpointCapabilities, EndpointClassification, EndpointType, RiskAssessment};

pub struct RiskScorer;

impl RiskScorer {
    pub fn score(
        classification: &EndpointClassification,
        capabilities: &EndpointCapabilities,
        auth_required: bool,
        anonymous: bool,
    ) -> RiskAssessment {
        let mut score: u16 = 0;
        let mut factors = Vec::new();

        if anonymous || !auth_required {
            score += 25;
            factors.push("anonymous_or_unauthenticated".into());
        } else {
            score += 10;
            factors.push("authenticated".into());
        }

        match classification.endpoint_type {
            EndpointType::AiChat | EndpointType::Completion => score += 15,
            EndpointType::AiAgent | EndpointType::Workflow => {
                score += 25;
                factors.push("agent_surface".into());
            }
            EndpointType::ToolEndpoint | EndpointType::Mcp => {
                score += 20;
                factors.push("tool_execution".into());
            }
            EndpointType::Embedding => score += 8,
            EndpointType::NonAi => score += 2,
            _ => score += 12,
        }

        if capabilities.supports_streaming {
            score += 8;
            factors.push("streaming".into());
        }
        if capabilities.supports_tools {
            score += 15;
            factors.push("tools".into());
        }
        if capabilities.supports_vision {
            score += 10;
            factors.push("vision".into());
        }
        if capabilities.supports_memory {
            score += 12;
            factors.push("memory".into());
        }
        if capabilities.supports_agent {
            score += 10;
            factors.push("agent".into());
        }
        if capabilities.supports_json_mode {
            score += 5;
            factors.push("json_mode".into());
        }

        if classification.confidence >= 0.85 {
            score += 5;
            factors.push("high_confidence_ai".into());
        }

        RiskAssessment {
            score: score.min(100) as u8,
            factors,
        }
    }
}
