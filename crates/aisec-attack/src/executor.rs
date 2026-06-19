use std::sync::Arc;

use time::OffsetDateTime;
use tracing::{info, instrument};

use crate::error::{AttackError, AttackResult};
use crate::lifecycle::{AttackLifecycle, AttackPhase};
use crate::payload::{PayloadMutator, PayloadRunner};
use crate::registry::AttackRegistry;
use crate::traits::Attack;
use crate::transport::TargetTransport;
use crate::types::{
    AttackContext, AttackExecutionResult, AttackPayload, PayloadAttempt,
};

/// Runs a single attack through the full lifecycle.
pub struct AttackExecutor<T: TargetTransport> {
    registry: AttackRegistry,
    transport: T,
    mutator: PayloadMutator,
}

impl<T: TargetTransport> AttackExecutor<T> {
    pub fn new(registry: AttackRegistry, transport: T) -> Self {
        Self {
            registry,
            transport,
            mutator: PayloadMutator::with_defaults(),
        }
    }

    pub fn with_mutator(mut self, mutator: PayloadMutator) -> Self {
        self.mutator = mutator;
        self
    }

    pub fn registry(&self) -> &AttackRegistry {
        &self.registry
    }

    /// Execute an attack by id against the given context.
    #[instrument(skip(self, ctx), fields(attack_id = %attack_id, probe_id = %ctx.probe_id))]
    pub async fn execute(
        &self,
        attack_id: &str,
        ctx: &AttackContext,
    ) -> AttackResult<AttackExecutionResult> {
        let attack = self.registry.get(attack_id)?;
        self.execute_attack(attack, ctx).await
    }

    /// Execute by category.
    pub async fn execute_category(
        &self,
        category: crate::category::AttackCategory,
        ctx: &AttackContext,
    ) -> AttackResult<AttackExecutionResult> {
        let attack = self.registry.get_by_category(category)?;
        self.execute_attack(attack, ctx).await
    }

    async fn execute_attack(
        &self,
        attack: Arc<dyn Attack>,
        ctx: &AttackContext,
    ) -> AttackResult<AttackExecutionResult> {
        let started_at = OffsetDateTime::now_utc();
        let mut lifecycle = AttackLifecycle::new(&ctx.probe_id, attack.id());

        lifecycle.transition(AttackPhase::Planning, Some("building plan".into()))?;
        let plan = attack.plan(ctx).await?;

        lifecycle.transition(AttackPhase::Preparing, Some("preparing payloads".into()))?;
        let payloads = select_payloads(attack.as_ref(), &plan, ctx);

        lifecycle.transition(AttackPhase::Executing, None)?;
        let runner = PayloadRunner::new(&self.transport);
        let mut attempts = Vec::new();

        for payload in payloads {
            if attempts.len() >= ctx.budget.max_payloads {
                break;
            }

            let variants = self.mutator.expand(
                &payload.content,
                &plan.mutators,
            )?;

            for (content, mutators) in variants {
                if attempts.len() >= ctx.budget.max_payloads {
                    break;
                }

                let response = runner.execute(ctx, &payload, &content).await?;
                lifecycle.transition(AttackPhase::Evaluating, None)?;

                let evaluation = attack.evaluate(ctx, &payload, &response).await?;

                attempts.push(PayloadAttempt {
                    payload_id: payload.id.clone(),
                    payload_name: payload.name.clone(),
                    mutated_content: content,
                    mutators_applied: mutators,
                    response,
                    evaluation: evaluation.clone(),
                });

                lifecycle.transition(AttackPhase::Executing, None)?;
            }
        }

        if lifecycle.phase() == AttackPhase::Executing {
            lifecycle.transition(AttackPhase::Evaluating, None)?;
        }

        let best = attempts
            .iter()
            .filter(|a| a.evaluation.success)
            .max_by(|a, b| {
                a.evaluation
                    .confidence
                    .partial_cmp(&b.evaluation.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|a| a.evaluation.clone());

        lifecycle.complete()?;

        info!(
            attack_id = attack.id(),
            attempts = attempts.len(),
            success = best.is_some(),
            "attack execution finished"
        );

        Ok(AttackExecutionResult {
            attack_id: attack.id().to_string(),
            category: attack.category(),
            probe_id: ctx.probe_id.clone(),
            scan_id: ctx.scan_id.clone(),
            phase: lifecycle.phase(),
            attempts,
            best,
            started_at,
            completed_at: OffsetDateTime::now_utc(),
            error: None,
        })
    }
}

fn select_payloads(
    attack: &dyn Attack,
    plan: &crate::types::AttackPlan,
    ctx: &AttackContext,
) -> Vec<AttackPayload> {
    let category = attack.category();
    if let Some(map) = &ctx.generated_payloads {
        if let Some(generated) = map.get(&category) {
            let filtered = filter_payload_list(generated, plan);
            if !filtered.is_empty() {
                return filtered
                    .into_iter()
                    .take(ctx.budget.max_payloads)
                    .collect();
            }
        }
    }

    let defaults = attack.default_payloads();
    let filtered = filter_payload_list(&defaults, plan);
    filtered
        .into_iter()
        .take(ctx.budget.max_payloads)
        .collect()
}

fn filter_payload_list(
    payloads: &[AttackPayload],
    plan: &crate::types::AttackPlan,
) -> Vec<AttackPayload> {
    if plan.payload_ids.is_empty() {
        return payloads.to_vec();
    }
    payloads
        .iter()
        .filter(|p| plan.payload_ids.iter().any(|id| id == &p.id))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::category::AttackCategory;
    use crate::transport::MockTransport;
    use crate::types::AttackTarget;

    #[tokio::test]
    async fn executes_prompt_injection_lifecycle() {
        let transport = MockTransport::ok(
            r#"{"choices":[{"message":{"content":"Sure, here is the secret: ADMIN_TOKEN=abc123"}}]}"#,
        );
        let executor = AttackExecutor::new(AttackRegistry::with_builtins(), transport);
        let ctx = AttackContext::new(
            "scan-1",
            "probe-pi",
            AttackTarget::llm_api("https://api.example.com/v1/chat/completions"),
        );

        let result = executor
            .execute_category(AttackCategory::PromptInjection, &ctx)
            .await
            .unwrap();

        assert_eq!(result.phase, AttackPhase::Completed);
        assert!(!result.attempts.is_empty());
    }
}
