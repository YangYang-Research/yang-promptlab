//! AgentTrace domain types.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceStatus {
    Ok,
    Error,
    #[default]
    Running,
}

impl TraceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
            Self::Running => "running",
        }
    }

    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "ok" | "success" => Self::Ok,
            "error" | "failed" => Self::Error,
            _ => Self::Running,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanKind {
    Capability,
    Llm,
    Tool,
    Agent,
    Other,
}

impl SpanKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Capability => "capability",
            Self::Llm => "llm",
            Self::Tool => "tool",
            Self::Agent => "agent",
            Self::Other => "other",
        }
    }

    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "capability" | "capability_classify" => Self::Capability,
            "llm" => Self::Llm,
            "tool" => Self::Tool,
            "agent" => Self::Agent,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStart {
    pub name: String,
    pub session_id: Option<String>,
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanStart {
    /// Span / function name. Leave empty and use [`crate::start_span!`] to
    /// auto-fill from the Rust caller function; set explicitly for closures/tools.
    pub name: String,
    pub kind: SpanKind,
    pub parent_span_id: Option<String>,
    #[serde(default)]
    pub inputs: Option<Value>,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpanEnd {
    /// Replaces the inputs recorded at span start. Use when the full wire
    /// request is only known after the call returns (e.g. capability routing,
    /// where the span must open before the LLM call for correct timing).
    #[serde(default)]
    pub inputs: Option<Value>,
    #[serde(default)]
    pub outputs: Option<Value>,
    pub status: TraceStatus,
    #[serde(default)]
    pub metrics: BTreeMap<String, f64>,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentRecord {
    pub id: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceSummary {
    pub id: String,
    pub experiment_id: String,
    pub experiment_name: String,
    pub name: String,
    pub session_id: Option<String>,
    pub status: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub latency_ms: Option<i64>,
    pub span_count: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<i64>,
    pub tags: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpanRecord {
    pub id: String,
    pub trace_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub kind: String,
    pub status: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub latency_ms: Option<i64>,
    pub inputs: Option<Value>,
    pub outputs: Option<Value>,
    pub metrics: BTreeMap<String, f64>,
    pub attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceDetail {
    pub trace: TraceSummary,
    pub spans: Vec<SpanRecord>,
}

#[derive(Debug, Clone, Default)]
pub struct ListTracesFilter {
    pub experiment: Option<String>,
    pub session_id: Option<String>,
    pub limit: Option<usize>,
}
