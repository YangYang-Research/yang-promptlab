//! One AgentTrace per non-Yazg completion.
//!
//! Yazg already writes a multi-span turn trace. This hook covers every other
//! agent that reaches [`crate::gateway::InferenceClient::complete_outcome`].

use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use promptlab_agenttrace::{
    soft_end_span, soft_end_trace, SharedAgentTrace, SpanEnd, SpanKind, SpanStart, TraceHandle,
    TraceStart, TraceStatus,
};
use serde_json::Value;

pub(crate) struct OpenTrace {
    trace: TraceHandle,
    span: Option<promptlab_agenttrace::SpanHandle>,
    started: Instant,
}

fn slot() -> &'static Mutex<Option<SharedAgentTrace>> {
    static SLOT: OnceLock<Mutex<Option<SharedAgentTrace>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

pub fn install_agent_trace(store: SharedAgentTrace) {
    if let Ok(mut guard) = slot().lock() {
        *guard = Some(store);
    }
}

fn store() -> Option<SharedAgentTrace> {
    slot().lock().ok().and_then(|guard| guard.clone())
}

/// Yazg turns are traced in `yazg_runtime`. Skip them here so a completion
/// is not stored twice.
fn should_trace(agent_id: &str) -> bool {
    let agent_id = agent_id.trim();
    !agent_id.is_empty() && agent_id != "yazg"
}

pub(crate) async fn open_completion_trace(agent_id: &str, inputs: Value) -> Option<OpenTrace> {
    if !should_trace(agent_id) {
        return None;
    }
    let store = store()?;
    let agent_id = agent_id.trim();
    let exp = match store.experiment(agent_id).await {
        Ok(exp) => exp,
        Err(err) => {
            tracing::warn!(error = %err, agent_id, "agenttrace experiment failed");
            return None;
        }
    };
    let mut tags = std::collections::BTreeMap::new();
    tags.insert("agent".into(), agent_id.to_string());
    let trace = match exp
        .start_trace(TraceStart {
            name: crate::token_usage::humanize_agent_id(agent_id),
            session_id: None,
            tags,
        })
        .await
    {
        Ok(trace) => trace,
        Err(err) => {
            tracing::warn!(error = %err, agent_id, "agenttrace start_trace failed");
            return None;
        }
    };
    let span = trace
        .span(SpanStart {
            name: "completion".into(),
            kind: SpanKind::Llm,
            parent_span_id: None,
            inputs: Some(inputs),
            attributes: std::collections::BTreeMap::new(),
        })
        .await
        .ok();
    Some(OpenTrace {
        trace,
        span,
        started: Instant::now(),
    })
}

pub(crate) async fn close_completion_trace(
    open: Option<OpenTrace>,
    ok: bool,
    outputs: Value,
    input_tokens: u64,
    output_tokens: u64,
) {
    let Some(open) = open else { return };
    let status = if ok { TraceStatus::Ok } else { TraceStatus::Error };
    let total = input_tokens.saturating_add(output_tokens);
    let mut metrics = std::collections::BTreeMap::new();
    metrics.insert("input_tokens".into(), input_tokens as f64);
    metrics.insert("output_tokens".into(), output_tokens as f64);
    metrics.insert("total_tokens".into(), total as f64);
    metrics.insert(
        "latency_ms".into(),
        open.started.elapsed().as_millis() as f64,
    );
    soft_end_span(
        open.span.as_ref(),
        SpanEnd {
            inputs: None,
            outputs: Some(outputs),
            status,
            metrics,
            attributes: std::collections::BTreeMap::new(),
        },
    )
    .await;
    soft_end_trace(Some(&open.trace), status).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use promptlab_agenttrace::AgentTrace;

    #[tokio::test]
    async fn records_non_yazg_completion_and_skips_yazg() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("agenttrace.db");
        let store = std::sync::Arc::new(AgentTrace::open(&path).await.expect("open"));
        install_agent_trace(store.clone());

        assert!(open_completion_trace("yazg", serde_json::json!({})).await.is_none());

        let open = open_completion_trace(
            "generate_prompt_agent",
            serde_json::json!({"messages": ["probe"]}),
        )
        .await
        .expect("trace");
        close_completion_trace(Some(open), true, serde_json::json!({"content": "ok"}), 3, 4).await;

        let traces = store
            .list_traces(promptlab_agenttrace::ListTracesFilter::default())
            .await
            .expect("list");
        assert_eq!(traces.len(), 1);
        assert_eq!(traces[0].experiment_name, "generate_prompt_agent");
        assert_eq!(traces[0].status, "ok");
        assert_eq!(traces[0].tags.get("agent").map(String::as_str), Some("generate_prompt_agent"));
        assert_eq!(traces[0].total_tokens, Some(7));
    }
}
