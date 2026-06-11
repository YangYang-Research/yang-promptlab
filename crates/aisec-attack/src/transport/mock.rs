use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::error::AttackResult;
use crate::transport::{TargetTransport, TransportRequest, TransportResponse};

/// In-memory transport for unit tests.
pub struct MockTransport {
    responses: Arc<Mutex<Vec<TransportResponse>>>,
    captured: Arc<Mutex<Vec<TransportRequest>>>,
}

impl MockTransport {
    pub fn ok(body: impl Into<String>) -> Self {
        Self::with_responses(vec![TransportResponse {
            status: 200,
            headers: HashMap::new(),
            body: body.into(),
            duration_ms: 1,
        }])
    }

    pub fn with_responses(responses: Vec<TransportResponse>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses)),
            captured: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn captured_requests(&self) -> Vec<TransportRequest> {
        self.captured.lock().unwrap().clone()
    }
}

#[async_trait]
impl TargetTransport for MockTransport {
    async fn send(&self, request: TransportRequest) -> AttackResult<TransportResponse> {
        self.captured.lock().unwrap().push(request);
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            return Ok(TransportResponse {
                status: 200,
                headers: HashMap::new(),
                body: "{}".into(),
                duration_ms: 1,
            });
        }
        let idx = 0;
        Ok(responses[idx.min(responses.len() - 1)].clone())
    }
}
