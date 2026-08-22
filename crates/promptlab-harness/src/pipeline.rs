use async_trait::async_trait;

use crate::error::HarnessResult;
use crate::models::{AttackRequest, NormalizedResponse};

/// Decision from a pre-execute interceptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterceptAction {
    Continue,
    Deny { reason: String },
}

/// Harness execution interceptor (plugin mutate/deny/redact).
#[async_trait]
pub trait HarnessInterceptor: Send + Sync {
    async fn pre_execute(&self, request: &mut AttackRequest) -> HarnessResult<InterceptAction>;

    async fn post_execute(
        &self,
        request: &AttackRequest,
        response: &mut NormalizedResponse,
    ) -> HarnessResult<()>;
}
