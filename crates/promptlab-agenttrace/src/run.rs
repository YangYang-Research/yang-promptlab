//! One open trace for the agent that started the run.
//!
//! Nested work (reflection, another agent, a later completion) joins that
//! trace instead of opening its own.

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::store::{SharedAgentTrace, TraceHandle};
use crate::types::{TraceStart, TraceStatus};

struct ActiveRun {
    trace: TraceHandle,
    failed: Arc<AtomicBool>,
}

tokio::task_local! {
    static ACTIVE_RUN: ActiveRun;
}

fn slot() -> &'static Mutex<Option<SharedAgentTrace>> {
    static SLOT: OnceLock<Mutex<Option<SharedAgentTrace>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

pub fn install_store(store: SharedAgentTrace) {
    if let Ok(mut guard) = slot().lock() {
        *guard = Some(store);
    }
}

fn store() -> Option<SharedAgentTrace> {
    slot().lock().ok().and_then(|guard| guard.clone())
}

pub fn current_trace() -> Option<TraceHandle> {
    ACTIVE_RUN.try_with(|run| run.trace.clone()).ok()
}

pub fn mark_run_failed() {
    let _ = ACTIVE_RUN.try_with(|run| run.failed.store(true, Ordering::Relaxed));
}

/// Bind `trace` for `fut`. Does not open or close it.
pub async fn with_bound_trace<T>(trace: TraceHandle, fut: impl Future<Output = T>) -> T {
    if current_trace().is_some() {
        return fut.await;
    }
    let run = ActiveRun {
        trace,
        failed: Arc::new(AtomicBool::new(false)),
    };
    ACTIVE_RUN.scope(run, fut).await
}

/// Open one trace for `agent_id` around `fut`.
///
/// A run already on this task is reused. No installed store means `fut` runs
/// with no trace.
pub async fn trace_agent_run<T>(agent_id: &str, fut: impl Future<Output = T>) -> T {
    if current_trace().is_some() {
        return fut.await;
    }
    let Some(store) = store() else {
        return fut.await;
    };
    let agent_id = agent_id.trim();
    if agent_id.is_empty() || agent_id == "yazg" {
        return fut.await;
    }
    let exp = match store.experiment(agent_id).await {
        Ok(exp) => exp,
        Err(err) => {
            tracing::warn!(error = %err, agent_id, "agenttrace experiment failed");
            return fut.await;
        }
    };
    let mut tags = std::collections::BTreeMap::new();
    tags.insert("agent".into(), agent_id.to_string());
    let trace = match exp
        .start_trace(TraceStart {
            name: agent_id.to_string(),
            session_id: None,
            tags,
        })
        .await
    {
        Ok(trace) => trace,
        Err(err) => {
            tracing::warn!(error = %err, agent_id, "agenttrace start_trace failed");
            return fut.await;
        }
    };
    let failed = Arc::new(AtomicBool::new(false));
    let run = ActiveRun {
        trace: trace.clone(),
        failed: failed.clone(),
    };
    let output = ACTIVE_RUN.scope(run, fut).await;
    let status = if failed.load(Ordering::Relaxed) {
        TraceStatus::Error
    } else {
        TraceStatus::Ok
    };
    if let Err(err) = trace.end(status).await {
        tracing::warn!(error = %err, agent_id, "agenttrace trace end failed");
    }
    output
}
