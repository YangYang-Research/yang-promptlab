//! Persist AI Runtime traffic events to SQLite and serve historical charts.

use promptlab_inference::{
    TrafficDirection, TrafficEvent, TrafficSnapshot, traffic_drain_pending,
    traffic_ensure_started, traffic_lifetime_totals, traffic_set_lifetime_totals,
    traffic_snapshot_from_events,
};
use promptlab_storage::repositories::RuntimeTrafficRepository;
use promptlab_storage::{CreateRuntimeTrafficEvent, Repositories};
use tauri::{AppHandle, Manager};
use tracing::{info, warn};

use crate::state::AppState;

const RETENTION_MS: u64 = 90 * 24 * 60 * 60 * 1000;
const FLUSH_INTERVAL_MS: u64 = 1_000;

pub async fn bootstrap_traffic_persistence(app: &AppHandle) {
    traffic_ensure_started();
    let state = app.state::<AppState>();
    let repos = state.repositories();

    match repos.runtime_traffic().counters().await {
        Ok(counters) => {
            traffic_set_lifetime_totals(
                counters.lifetime_sent.max(0) as u64,
                counters.lifetime_received.max(0) as u64,
            );
            info!(
                lifetime_sent = counters.lifetime_sent,
                lifetime_received = counters.lifetime_received,
                "hydrated runtime traffic counters from db"
            );
        }
        Err(err) => warn!(error = %err, "failed to hydrate runtime traffic counters"),
    }

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let cutoff = now_ms.saturating_sub(RETENTION_MS as i64);
    match repos.runtime_traffic().prune_before(cutoff).await {
        Ok(removed) if removed > 0 => {
            info!(removed, cutoff_ms = cutoff, "pruned old runtime traffic events");
        }
        Ok(_) => {}
        Err(err) => warn!(error = %err, "failed to prune runtime traffic events"),
    }

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(FLUSH_INTERVAL_MS)).await;
            let state = app_handle.state::<AppState>();
            if let Err(err) = flush_pending_traffic(&state.repositories()).await {
                warn!(error = %err, "runtime traffic flush failed");
            }
        }
    });
}

pub async fn flush_pending_traffic(repos: &Repositories) -> promptlab_core::PromptLabResult<u64> {
    let pending = traffic_drain_pending();
    if pending.is_empty() {
        return Ok(0);
    }
    let events = pending
        .into_iter()
        .map(|event| CreateRuntimeTrafficEvent {
            at_ms: event.at_ms as i64,
            direction: event.direction.as_str().to_string(),
        })
        .collect::<Vec<_>>();
    repos.runtime_traffic().insert_many(events).await
}

pub async fn traffic_snapshot_from_db(
    repos: &Repositories,
    window_ms: u64,
    bucket_ms: u64,
) -> promptlab_core::PromptLabResult<TrafficSnapshot> {
    let _ = flush_pending_traffic(repos).await?;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let start_ms = if window_ms == 0 {
        0
    } else {
        now_ms.saturating_sub(window_ms.clamp(5_000, RETENTION_MS))
    };

    let rows = repos
        .runtime_traffic()
        .list_between(start_ms as i64, now_ms as i64)
        .await?;
    let events = rows
        .into_iter()
        .filter_map(|row| {
            let direction = TrafficDirection::parse(&row.direction)?;
            Some(TrafficEvent {
                at_ms: row.at_ms.max(0) as u64,
                direction,
            })
        })
        .collect::<Vec<_>>();

    let counters = repos.runtime_traffic().counters().await?;
    let (mem_sent, mem_received) = traffic_lifetime_totals();
    let lifetime_sent = (counters.lifetime_sent.max(0) as u64).max(mem_sent);
    let lifetime_received = (counters.lifetime_received.max(0) as u64).max(mem_received);

    Ok(traffic_snapshot_from_events(
        &events,
        window_ms,
        bucket_ms,
        lifetime_sent,
        lifetime_received,
    ))
}
