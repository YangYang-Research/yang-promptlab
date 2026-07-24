use async_trait::async_trait;

use crate::category::AttackCategory;
use crate::error::AttackResult;
use crate::payload::MutatorKind;
use crate::types::{
    AttackContext, AttackEvaluation, AttackPayload, AttackPlan, AttackResponse,
};

/// Contract for a pluggable attack implementation.
#[async_trait]
pub trait Attack: Send + Sync {
    /// Stable attack identifier (e.g. `prompt_injection`).
    fn id(&self) -> &'static str;

    fn category(&self) -> AttackCategory;

    fn name(&self) -> &'static str;

    fn description(&self) -> &'static str;

    /// Produce an execution plan for the given context.
    async fn plan(&self, ctx: &AttackContext) -> AttackResult<AttackPlan>;

    /// Built-in payloads for this attack category.
    fn default_payloads(&self) -> Vec<AttackPayload>;

    /// Mutators recommended for this attack.
    fn supported_mutators(&self) -> Vec<MutatorKind> {
        vec![]
    }

    /// Evaluate target response for vulnerability indicators.
    async fn evaluate(
        &self,
        ctx: &AttackContext,
        payload: &AttackPayload,
        response: &AttackResponse,
    ) -> AttackResult<AttackEvaluation>;
}
