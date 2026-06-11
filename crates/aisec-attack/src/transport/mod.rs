use std::collections::HashMap;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{AttackError, AttackResult};

/// Outbound request to the target under test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportRequest {
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub timeout_ms: u64,
}

/// Inbound response from the target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub duration_ms: u64,
}

/// Abstraction for delivering attack payloads to targets.
#[async_trait]
pub trait TargetTransport: Send + Sync {
    async fn send(&self, request: TransportRequest) -> AttackResult<TransportResponse>;
}

mod http;
mod mock;

pub use http::HttpTransport;
pub use mock::MockTransport;
