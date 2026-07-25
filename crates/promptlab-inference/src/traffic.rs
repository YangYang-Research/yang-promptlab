//! Inference traffic counters for runtime monitoring charts.
//!
//! Hot-path recording stays in-memory and queues events for DB persistence.
//! Historical charts are built from persisted events via [`snapshot_from_events`].
//!
//! **Choke points (must record here, not in feature crates):**
//! - [`crate::provider::RemoteProviderAdapter::complete`] / [`crate::provider::LlamaCppAdapter::complete`]
//!   — all gateway / agent / judge-via-adapter completions
//! - Judge legacy backends (`promptlab-judge` `RemoteLlmBackend` / `LocalLlmBackend`)
//! - Health/connectivity wrappers that call `health` without going through `complete`
//!   ([`crate::gateway::GatewaySession::health`], `test_remote_connectivity_only`)
//!
//! Do not also record in `GatewaySession::complete` / judge `AdapterRuntime` — that double-counts.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Cap retained in-memory samples (DB holds the durable history).
const MAX_EVENTS: usize = 4_096;
/// Cap chart buckets so long sessions stay renderable.
const MAX_BUCKETS: usize = 240;
/// Longest supported rolling window (3 months).
const MAX_WINDOW_MS: u64 = 90 * 24 * 60 * 60 * 1000;
/// Widest bucket for long-range charts.
const MAX_BUCKET_MS: u64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrafficDirection {
    Sent,
    Received,
}

impl TrafficDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sent => "sent",
            Self::Received => "received",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "sent" => Some(Self::Sent),
            "received" => Some(Self::Received),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TrafficEvent {
    pub at_ms: u64,
    pub direction: TrafficDirection,
}

#[derive(Debug, Default)]
struct TrafficState {
    events: VecDeque<TrafficEvent>,
    pending: VecDeque<TrafficEvent>,
    lifetime_sent: u64,
    lifetime_received: u64,
    started_at_ms: Option<u64>,
}

/// Time-bucketed series for UI charts.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrafficBucket {
    pub at_ms: u64,
    pub sent: u64,
    pub received: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrafficSnapshot {
    pub window_ms: u64,
    pub bucket_ms: u64,
    pub buckets: Vec<TrafficBucket>,
    pub total_sent: u64,
    pub total_received: u64,
    /// True when the series covers the full session (not a rolling window).
    pub continuous: bool,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn monitor() -> &'static Mutex<TrafficState> {
    static MONITOR: OnceLock<Mutex<TrafficState>> = OnceLock::new();
    MONITOR.get_or_init(|| Mutex::new(TrafficState::default()))
}

fn push_event(direction: TrafficDirection) {
    let Ok(mut state) = monitor().lock() else {
        return;
    };
    let at_ms = now_ms();
    if state.started_at_ms.is_none() {
        state.started_at_ms = Some(at_ms);
    }
    match direction {
        TrafficDirection::Sent => state.lifetime_sent = state.lifetime_sent.saturating_add(1),
        TrafficDirection::Received => {
            state.lifetime_received = state.lifetime_received.saturating_add(1)
        }
    }
    let event = TrafficEvent { at_ms, direction };
    state.events.push_back(event);
    state.pending.push_back(event);
    while state.events.len() > MAX_EVENTS {
        state.events.pop_front();
    }
}

/// Start the continuous traffic timeline at app boot (before any packages).
pub fn ensure_started() {
    let Ok(mut state) = monitor().lock() else {
        return;
    };
    if state.started_at_ms.is_none() {
        state.started_at_ms = Some(now_ms());
    }
}

/// Hydrate lifetime counters from durable storage after process restart.
pub fn set_lifetime_totals(sent: u64, received: u64) {
    let Ok(mut state) = monitor().lock() else {
        return;
    };
    state.lifetime_sent = sent;
    state.lifetime_received = received;
}

pub fn lifetime_totals() -> (u64, u64) {
    let Ok(state) = monitor().lock() else {
        return (0, 0);
    };
    (state.lifetime_sent, state.lifetime_received)
}

/// Drain events waiting to be persisted.
pub fn drain_pending() -> Vec<TrafficEvent> {
    let Ok(mut state) = monitor().lock() else {
        return Vec::new();
    };
    state.pending.drain(..).collect()
}

/// Record one outbound inference package (request).
pub fn record_sent() {
    push_event(TrafficDirection::Sent);
}

/// Record one inbound inference package (response).
pub fn record_received() {
    push_event(TrafficDirection::Received);
}

/// Record a completed request/response round-trip.
pub fn record_roundtrip() {
    record_sent();
    record_received();
}

/// Snapshot from an arbitrary event list (typically loaded from DB).
pub fn snapshot_from_events(
    events: &[TrafficEvent],
    window_ms: u64,
    bucket_ms: u64,
    lifetime_sent: u64,
    lifetime_received: u64,
) -> TrafficSnapshot {
    ensure_started();

    let continuous = window_ms == 0;
    let requested_bucket_ms = bucket_ms.clamp(1_000, MAX_BUCKET_MS);
    let now = now_ms();
    let rolling_window_ms = if continuous {
        0
    } else {
        window_ms.clamp(5_000, MAX_WINDOW_MS)
    };

    let start = if continuous {
        events
            .first()
            .map(|event| event.at_ms)
            .or_else(|| {
                monitor()
                    .lock()
                    .ok()
                    .and_then(|state| state.started_at_ms)
            })
            .unwrap_or(now)
    } else {
        now.saturating_sub(rolling_window_ms)
    };

    let span_ms = if continuous {
        now.saturating_sub(start).max(requested_bucket_ms)
    } else {
        rolling_window_ms
    };
    let mut resolved_bucket_ms = requested_bucket_ms;
    let mut bucket_count = ((span_ms + resolved_bucket_ms - 1) / resolved_bucket_ms).max(1) as usize;
    if bucket_count > MAX_BUCKETS {
        resolved_bucket_ms = (span_ms / MAX_BUCKETS as u64).max(1_000);
        bucket_count = ((span_ms + resolved_bucket_ms - 1) / resolved_bucket_ms).max(1) as usize;
        bucket_count = bucket_count.min(MAX_BUCKETS);
    }

    let mut sent = vec![0u64; bucket_count];
    let mut received = vec![0u64; bucket_count];

    for event in events {
        if event.at_ms < start || event.at_ms > now {
            continue;
        }
        let index = ((event.at_ms - start) / resolved_bucket_ms) as usize;
        let index = index.min(bucket_count - 1);
        match event.direction {
            TrafficDirection::Sent => sent[index] += 1,
            TrafficDirection::Received => received[index] += 1,
        }
    }

    let buckets = (0..bucket_count)
        .map(|index| {
            let at_ms = if bucket_count == 1 {
                start
            } else {
                start + (index as u64 * span_ms) / (bucket_count as u64 - 1)
            };
            TrafficBucket {
                at_ms,
                sent: sent[index],
                received: received[index],
            }
        })
        .collect();

    TrafficSnapshot {
        window_ms: span_ms,
        bucket_ms: resolved_bucket_ms,
        buckets,
        total_sent: lifetime_sent,
        total_received: lifetime_received,
        continuous,
    }
}

/// Snapshot traffic into fixed-width buckets using the in-memory ring (tests / fallback).
pub fn snapshot(window_ms: u64, bucket_ms: u64) -> TrafficSnapshot {
    ensure_started();
    let (events, lifetime_sent, lifetime_received) = {
        let Ok(state) = monitor().lock() else {
            return snapshot_from_events(&[], window_ms, bucket_ms, 0, 0);
        };
        (
            state.events.iter().copied().collect::<Vec<_>>(),
            state.lifetime_sent,
            state.lifetime_received,
        )
    };
    snapshot_from_events(&events, window_ms, bucket_ms, lifetime_sent, lifetime_received)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_counts_lifetime_totals_with_rolling_window() {
        ensure_started();
        record_roundtrip();
        record_roundtrip();
        let snap = snapshot(60_000, 1_000);
        assert!(!snap.continuous);
        assert!(snap.total_sent >= 2);
        assert!(snap.total_received >= 2);
        assert!(!snap.buckets.is_empty());
        assert_eq!(snap.window_ms, 60_000);
        assert_eq!(snap.bucket_ms, 1_000);
    }

    #[test]
    fn rolling_window_starts_from_now_not_boot() {
        ensure_started();
        let snap = snapshot(60_000, 1_000);
        assert!(!snap.continuous);
        assert!(!snap.buckets.is_empty());
        let first = snap.buckets[0].at_ms;
        let last = snap.buckets[snap.buckets.len() - 1].at_ms;
        assert!(last >= first);
        assert_eq!(last.saturating_sub(first), 60_000);
    }

    #[test]
    fn long_range_window_is_accepted() {
        ensure_started();
        let snap = snapshot(90 * 24 * 60 * 60 * 1000, 6 * 60 * 60 * 1000);
        assert!(!snap.continuous);
        assert!(!snap.buckets.is_empty());
        assert_eq!(snap.window_ms, 90 * 24 * 60 * 60 * 1000);
    }

    #[test]
    fn snapshot_from_events_buckets_by_second() {
        let now = now_ms();
        let events = [
            TrafficEvent {
                at_ms: now.saturating_sub(2_000),
                direction: TrafficDirection::Sent,
            },
            TrafficEvent {
                at_ms: now.saturating_sub(2_000),
                direction: TrafficDirection::Sent,
            },
            TrafficEvent {
                at_ms: now.saturating_sub(2_000),
                direction: TrafficDirection::Sent,
            },
            TrafficEvent {
                at_ms: now.saturating_sub(1_000),
                direction: TrafficDirection::Received,
            },
        ];
        let snap = snapshot_from_events(&events, 5_000, 1_000, 3, 1);
        assert_eq!(snap.total_sent, 3);
        assert_eq!(snap.total_received, 1);
        assert!(snap.buckets.iter().any(|bucket| bucket.sent == 3));
        assert!(snap.buckets.iter().any(|bucket| bucket.received == 1));
    }
}
