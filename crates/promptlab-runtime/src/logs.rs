//! Ring buffer for runtime operational logs.

use std::collections::VecDeque;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLogEntry {
    #[serde(with = "time::serde::rfc3339")]
    pub timestamp: OffsetDateTime,
    pub level: String,
    pub message: String,
}

#[derive(Clone, Default)]
pub struct RuntimeLogs {
    inner: Arc<Mutex<VecDeque<RuntimeLogEntry>>>,
    capacity: usize,
}

impl RuntimeLogs {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::new())),
            capacity,
        }
    }

    pub async fn push(&self, level: impl Into<String>, message: impl Into<String>) {
        let mut buf = self.inner.lock().await;
        if buf.len() >= self.capacity {
            buf.pop_front();
        }
        buf.push_back(RuntimeLogEntry {
            timestamp: OffsetDateTime::now_utc(),
            level: level.into(),
            message: message.into(),
        });
    }

    pub async fn entries(&self, limit: usize) -> Vec<RuntimeLogEntry> {
        let buf = self.inner.lock().await;
        buf.iter().rev().take(limit).cloned().collect()
    }
}
