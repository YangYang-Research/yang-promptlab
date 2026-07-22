use serde::{Deserialize, Serialize};

/// Stable agent identities in the Yazg hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentId {
    Yazg,
    AnalyzeEndpoint,
    AttackPlan,
    GeneratePrompt,
    Recommend,
    Summary,
}

impl AgentId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Yazg => "yazg",
            Self::AnalyzeEndpoint => "analyze_endpoint",
            Self::AttackPlan => "attack_plan",
            Self::GeneratePrompt => "generate_prompt",
            Self::Recommend => "recommend",
            Self::Summary => "summary",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Yazg => "Yazg",
            Self::AnalyzeEndpoint => "AnalyzeEndpointAgent",
            Self::AttackPlan => "AttackPlanAgent",
            Self::GeneratePrompt => "GeneratePromptAgent",
            Self::Recommend => "RecommendAgent",
            Self::Summary => "SummaryAgent",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentEventKind {
    Started,
    Completed,
    Failed,
    Info,
}

/// Timeline event emitted while Yazg delegates to sub-agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEvent {
    pub agent: AgentId,
    pub kind: AgentEventKind,
    pub message: String,
}

impl AgentEvent {
    pub fn started(agent: AgentId, message: impl Into<String>) -> Self {
        Self {
            agent,
            kind: AgentEventKind::Started,
            message: message.into(),
        }
    }

    pub fn completed(agent: AgentId, message: impl Into<String>) -> Self {
        Self {
            agent,
            kind: AgentEventKind::Completed,
            message: message.into(),
        }
    }

    pub fn failed(agent: AgentId, message: impl Into<String>) -> Self {
        Self {
            agent,
            kind: AgentEventKind::Failed,
            message: message.into(),
        }
    }

    pub fn info(agent: AgentId, message: impl Into<String>) -> Self {
        Self {
            agent,
            kind: AgentEventKind::Info,
            message: message.into(),
        }
    }
}
