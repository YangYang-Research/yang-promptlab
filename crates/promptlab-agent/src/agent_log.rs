//! Central agent activity logging → tracing + `~/.promptlab/logs/agents.log`.

use promptlab_core::{global_event_bus, LogCategory, OcsfEvent, OcsfSeverity};
use tracing::{debug, error, info, warn};

use crate::types::{AgentEvent, AgentEventKind, AgentId};

const MODULE: &str = "promptlab-agent";

/// Persist + emit a structured agent timeline event.
pub fn log_agent_event(event: &AgentEvent) {
    let agent = event.agent.as_str();
    let kind = event.kind.as_str();
    let message = event.message.as_str();

    match event.kind {
        AgentEventKind::Failed => {
            error!(agent, kind, message, "agent event");
        }
        AgentEventKind::Started | AgentEventKind::Completed => {
            info!(agent, kind, message, "agent event");
        }
        AgentEventKind::React | AgentEventKind::ToolCall | AgentEventKind::Llm => {
            info!(agent, kind, message, "agent event");
        }
        AgentEventKind::Info => {
            debug!(agent, kind, message, "agent event");
            // Also surface Info at info level when it looks like an action.
            if message.starts_with("Thought:")
                || message.starts_with("Action:")
                || message.starts_with("Observation:")
                || message.starts_with("Acting:")
                || message.starts_with("ReAct")
            {
                info!(agent, kind, message, "agent action");
            }
        }
    }

    publish_ocsf(event, None, None, None);
}

/// Log a ReAct thought / action / observation line.
pub fn log_react(
    agent: AgentId,
    phase: &str,
    detail: impl Into<String>,
    context: AgentLogContext<'_>,
) {
    let detail = detail.into();
    let message = format!("ReAct {phase}: {detail}");
    info!(
        agent = agent.as_str(),
        phase,
        detail = %truncate(&detail, 800),
        scan_id = context.scan_id.unwrap_or(""),
        "agent react"
    );
    let event = AgentEvent {
        agent,
        kind: AgentEventKind::React,
        message,
    };
    publish_ocsf(
        &event,
        context.project_id,
        context.target_id,
        context.scan_id,
    );
}

/// Log a tool / host harness invocation (generate, attack, recover, create_project, …).
pub fn log_tool_call(
    agent: AgentId,
    tool: &str,
    detail: impl Into<String>,
    context: AgentLogContext<'_>,
) {
    let detail = detail.into();
    let message = format!("tool:{tool} {detail}");
    info!(
        agent = agent.as_str(),
        tool,
        detail = %truncate(&detail, 800),
        scan_id = context.scan_id.unwrap_or(""),
        "agent tool_call"
    );
    let event = AgentEvent {
        agent,
        kind: AgentEventKind::ToolCall,
        message,
    };
    publish_ocsf(
        &event,
        context.project_id,
        context.target_id,
        context.scan_id,
    );
}

/// Log an LLM complete round-trip (prompt/response truncated).
pub fn log_llm_call(
    agent: AgentId,
    role: &str,
    prompt_chars: usize,
    response: &str,
    ok: bool,
    context: AgentLogContext<'_>,
) {
    let preview = truncate(response, 500);
    let message = if ok {
        format!("llm:{role} ok prompt_chars={prompt_chars} response={preview}")
    } else {
        format!("llm:{role} error prompt_chars={prompt_chars} response={preview}")
    };
    if ok {
        info!(
            agent = agent.as_str(),
            role,
            prompt_chars,
            response = %preview,
            "agent llm"
        );
    } else {
        warn!(
            agent = agent.as_str(),
            role,
            prompt_chars,
            response = %preview,
            "agent llm failed"
        );
    }
    let event = AgentEvent {
        agent,
        kind: AgentEventKind::Llm,
        message,
    };
    publish_ocsf(
        &event,
        context.project_id,
        context.target_id,
        context.scan_id,
    );
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AgentLogContext<'a> {
    pub project_id: Option<&'a str>,
    pub target_id: Option<&'a str>,
    pub scan_id: Option<&'a str>,
}

impl<'a> AgentLogContext<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_scan(mut self, scan_id: Option<&'a str>) -> Self {
        self.scan_id = scan_id;
        self
    }

    pub fn with_project(mut self, project_id: Option<&'a str>) -> Self {
        self.project_id = project_id;
        self
    }

    pub fn with_target(mut self, target_id: Option<&'a str>) -> Self {
        self.target_id = target_id;
        self
    }
}

fn publish_ocsf(
    event: &AgentEvent,
    project_id: Option<&str>,
    _target_id: Option<&str>,
    scan_id: Option<&str>,
) {
    let Some(bus) = global_event_bus() else {
        return;
    };
    let severity = match event.kind {
        AgentEventKind::Failed => OcsfSeverity::High,
        AgentEventKind::Started | AgentEventKind::Completed => OcsfSeverity::Informational,
        AgentEventKind::React | AgentEventKind::ToolCall | AgentEventKind::Llm => {
            OcsfSeverity::Informational
        }
        AgentEventKind::Info => OcsfSeverity::Informational,
    };
    let ocsf = OcsfEvent::new(
        LogCategory::Agent,
        severity,
        event.kind.as_str(),
        MODULE,
        event.agent.as_str(),
        &event.message,
    )
    .with_context(
        None,
        project_id.map(str::to_string),
        scan_id.map(str::to_string),
    )
    .attr("agent", event.agent.as_str())
    .attr("agentDisplay", event.agent.display_name())
    .attr("eventKind", event.kind.as_str());
    bus.publish(ocsf);
}

fn truncate(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    let shortened: String = t.chars().take(max.saturating_sub(1)).collect();
    format!("{shortened}…")
}
