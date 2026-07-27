use serde::{Deserialize, Serialize};

use crate::agent_log::log_agent_event;

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
    JudgeCoordinator,
    JudgeWorker,
    ClassifierWorker,
    AttackerWorker,
    AgenticAttackExecution,
    SequentialAttackExecution,
    Reflection,
    CreateProject,
    ListWorkspace,
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
            Self::JudgeCoordinator => "judge_coordinator",
            Self::JudgeWorker => "judge_worker",
            Self::ClassifierWorker => "classifier_worker",
            Self::AttackerWorker => "attacker_worker",
            Self::AgenticAttackExecution => "agentic_attack_execution",
            Self::SequentialAttackExecution => "sequential_attack_execution",
            Self::Reflection => "reflection",
            Self::CreateProject => "create_project",
            Self::ListWorkspace => "list_workspace",
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
            Self::JudgeCoordinator => "JudgeCoordinatorAgent",
            Self::JudgeWorker => "JudgeWorker",
            Self::ClassifierWorker => "ClassifierWorker",
            Self::AttackerWorker => "AttackerWorker",
            Self::AgenticAttackExecution => "AgenticAttackExecutionAgent",
            Self::SequentialAttackExecution => "SequentialAttackExecutionAgent",
            Self::Reflection => "ReflectionAgent",
            Self::CreateProject => "CreateProjectTool",
            Self::ListWorkspace => "ListWorkspaceTool",
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
    /// ReAct thought / action / observation.
    React,
    /// Host tool or sub-agent invocation.
    ToolCall,
    /// LLM complete round-trip.
    Llm,
}

impl AgentEventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Info => "info",
            Self::React => "react",
            Self::ToolCall => "tool_call",
            Self::Llm => "llm",
        }
    }
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
    fn emit(self) -> Self {
        log_agent_event(&self);
        self
    }

    pub fn started(agent: AgentId, message: impl Into<String>) -> Self {
        Self {
            agent,
            kind: AgentEventKind::Started,
            message: message.into(),
        }
        .emit()
    }

    pub fn completed(agent: AgentId, message: impl Into<String>) -> Self {
        Self {
            agent,
            kind: AgentEventKind::Completed,
            message: message.into(),
        }
        .emit()
    }

    pub fn failed(agent: AgentId, message: impl Into<String>) -> Self {
        Self {
            agent,
            kind: AgentEventKind::Failed,
            message: message.into(),
        }
        .emit()
    }

    pub fn info(agent: AgentId, message: impl Into<String>) -> Self {
        Self {
            agent,
            kind: AgentEventKind::Info,
            message: message.into(),
        }
        .emit()
    }

    pub fn react(agent: AgentId, message: impl Into<String>) -> Self {
        Self {
            agent,
            kind: AgentEventKind::React,
            message: message.into(),
        }
        .emit()
    }

    pub fn tool_call(agent: AgentId, message: impl Into<String>) -> Self {
        Self {
            agent,
            kind: AgentEventKind::ToolCall,
            message: message.into(),
        }
        .emit()
    }

    pub fn llm(agent: AgentId, message: impl Into<String>) -> Self {
        Self {
            agent,
            kind: AgentEventKind::Llm,
            message: message.into(),
        }
        .emit()
    }
}
