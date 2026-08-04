//! AgentTrace — GenAI span tracing for PromptLab agents.
//!
//! Hierarchy: Experiment → Trace → Span (nested via `parent_span_id`).
//! Sessions group traces by conversation (`yazg-chat:<threadId>`).
//!
//! Prefer [`start_span!`] so span `name` is the Rust caller function
//! (leave `SpanStart::name` empty). Override `name` only for closures / tools.

mod error;
pub mod fn_name;
mod store;
mod types;

pub use error::{AgentTraceError, Result};
pub use store::{
    soft_end_span, soft_end_trace, soft_start_span, AgentTrace, ExperimentHandle, SessionSummary,
    SharedAgentTrace, SpanHandle, TraceHandle,
};
pub use types::{
    ExperimentRecord, ListTracesFilter, SpanEnd, SpanKind, SpanRecord, SpanStart, TraceDetail,
    TraceStart, TraceStatus, TraceSummary,
};
