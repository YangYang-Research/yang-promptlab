use std::collections::HashMap;

use async_trait::async_trait;
use aisec_harness::NormalizedResponse;
use serde::{Deserialize, Serialize};

use crate::error::AttackResult;

/// Outbound request to the target under test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportRequest {
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub timeout_ms: u64,
}

/// Inbound response from the target, including harness-normalized payload for the judge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub duration_ms: u64,
    pub normalized: NormalizedResponse,
}

/// Abstraction for delivering attack payloads to targets.
///
/// Production implementations must route through [`HarnessTransport`].
#[async_trait]
pub trait TargetTransport: Send + Sync {
    async fn send(&self, request: TransportRequest) -> AttackResult<TransportResponse>;
}

mod harness;
mod mock;

pub use harness::HarnessTransport;
pub use mock::MockTransport;
