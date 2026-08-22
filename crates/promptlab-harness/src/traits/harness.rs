use async_trait::async_trait;

use crate::error::HarnessResult;
use crate::models::{AttackRequest, NormalizedResponse};

/// Target I/O adapter. New protocols register an impl; they do not add match-arms
/// in attack/verify/discovery callers.
#[async_trait]
pub trait Harness: Send + Sync {
    fn id(&self) -> &'static str;

    async fn execute(&self, request: AttackRequest) -> HarnessResult<NormalizedResponse>;
}
