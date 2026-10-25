//! LLM spans on the agent run that is already open.
//!
//! A completion does not open its own trace. Reflection, retries, and later
//! model calls stay inside the trace started by [`promptlab_agenttrace::trace_agent_run`].
//! Yazg records its own spans in `yazg_runtime`.

use std::time::Instant;

use promptlab_agenttrace::{SpanEnd, SpanKind, SpanStart, TraceStatus};
use serde_json::Value;

pub(crate) struct OpenSpan {
    span: Option<promptlab_agenttrace::SpanHandle>,
    started: Instant,
}

pub fn install_agent_trace(store: promptlab_agenttrace::SharedAgentTrace) {
    promptlab_agenttrace::install_store(store);
}

pub(crate) async fn open_completion_span(agent_id: &str, inputs: Value) -> Option<OpenSpan> {
    let agent_id = agent_id.trim();
    if agent_id.is_empty() || agent_id == "yazg" {
        return None;
    }
    let trace = promptlab_agenttrace::current_trace()?;
    let mut attributes = std::collections::BTreeMap::new();
    attributes.insert("agent".into(), agent_id.to_string());
    let span = trace
        .span(SpanStart {
            name: agent_id.to_string(),
            kind: SpanKind::Llm,
            parent_span_id: None,
            inputs: Some(inputs),
            attributes,
        })
        .await
        .ok();
    Some(OpenSpan {
        span,
        started: Instant::now(),
    })
}

pub(crate) async fn close_completion_span(
    open: Option<OpenSpan>,
    ok: bool,
    outputs: Value,
    input_tokens: u64,
    output_tokens: u64,
) {
    let Some(open) = open else { return };
    if !ok {
        promptlab_agenttrace::mark_run_failed();
    }
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
    promptlab_agenttrace::soft_end_span(
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use promptlab_agenttrace::AgentTrace;

    #[tokio::test]
    async fn one_run_keeps_reflection_on_the_same_trace() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("agenttrace.db");
        let store = std::sync::Arc::new(AgentTrace::open(&path).await.expect("open"));
        install_agent_trace(store.clone());

        promptlab_agenttrace::trace_agent_run("jailbreak_agent", async {
            assert!(open_completion_span("yazg", serde_json::json!({})).await.is_none());
            let first = open_completion_span("jailbreak_agent", serde_json::json!({"step": "followup"}))
                .await
                .expect("span");
            close_completion_span(Some(first), true, serde_json::json!({"content": "draft"}), 2, 2).await;
            promptlab_agenttrace::trace_agent_run("reflection_agent", async {
                let nested = open_completion_span(
                    "reflection_agent",
                    serde_json::json!({"step": "reflect"}),
                )
                .await
                .expect("nested span");
                close_completion_span(Some(nested), true, serde_json::json!({"accept": true}), 1, 1)
                    .await;
            })
            .await;
        })
        .await;

        let traces = store
            .list_traces(promptlab_agenttrace::ListTracesFilter::default())
            .await
            .expect("list");
        assert_eq!(traces.len(), 1);
        assert_eq!(traces[0].experiment_name, "jailbreak_agent");
        assert_eq!(traces[0].span_count, 2);
        assert_eq!(traces[0].total_tokens, Some(6));
        let detail = store
            .get_trace(&traces[0].id)
            .await
            .expect("detail")
            .expect("present");
        let names: Vec<&str> = detail.spans.iter().map(|span| span.name.as_str()).collect();
        assert_eq!(names, ["jailbreak_agent", "reflection_agent"]);
    }
}
