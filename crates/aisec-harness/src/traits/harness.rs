use async_trait::async_trait;

use crate::error::HarnessResult;
use crate::models::{AttackRequest, NormalizedResponse};

/// Executes attacks against a specific target transport.
#[async_trait]
pub trait Harness: Send + Sync {
    fn id(&self) -> &'static str;

    async fn execute(&self, request: AttackRequest) -> HarnessResult<NormalizedResponse>;
}
