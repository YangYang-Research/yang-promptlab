use std::collections::HashMap;
use std::sync::Arc;

use promptlab_harness::NormalizedResponse;
use time::OffsetDateTime;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tracing::{info, instrument};

use crate::error::{AttackError, AttackResult};
use crate::lifecycle::{AttackLifecycle, AttackPhase};
use crate::payload::{MutatorKind, PayloadMutator, PayloadRunner};
use crate::registry::AttackRegistry;
use crate::traits::Attack;
use crate::transport::TargetTransport;
use crate::types::{
    AttackContext, AttackEvaluation, AttackExecutionResult, AttackPayload, AttackResponse,
    PayloadAttempt,
};

pub type AttemptStreamItem = (usize, PayloadAttempt);

/// Runs a single attack through the full lifecycle.
pub struct AttackExecutor<T: TargetTransport> {
    registry: AttackRegistry,
    transport: T,
    mutator: PayloadMutator,
}

impl<T: TargetTransport + Clone + 'static> AttackExecutor<T> {
    pub fn new(registry: AttackRegistry, transport: T) -> Self {
        Self {
            registry,
            transport,
            // Never silently expand — scan paths set mutator from variantsPerTest.
            mutator: PayloadMutator::identity(),
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
        self.execute_attack(attack, ctx, None).await
    }

    /// Execute by category.
    pub async fn execute_category(
        &self,
        category: crate::category::AttackCategory,
        ctx: &AttackContext,
    ) -> AttackResult<AttackExecutionResult> {
        let attack = self.registry.get_by_category(category)?;
        self.execute_attack(attack, ctx, None).await
    }

    /// Execute by category, emitting each completed attempt as HTTP finishes (pool-limited).
    pub async fn execute_category_streaming(
        &self,
        category: crate::category::AttackCategory,
        ctx: &AttackContext,
        attempt_tx: mpsc::Sender<AttemptStreamItem>,
    ) -> AttackResult<AttackExecutionResult> {
        let attack = self.registry.get_by_category(category)?;
        self.execute_attack(attack, ctx, Some(attempt_tx)).await
    }

    async fn execute_attack(
        &self,
        attack: Arc<dyn Attack>,
        ctx: &AttackContext,
        attempt_tx: Option<mpsc::Sender<AttemptStreamItem>>,
    ) -> AttackResult<AttackExecutionResult> {
        let started_at = OffsetDateTime::now_utc();
        let mut lifecycle = AttackLifecycle::new(&ctx.probe_id, attack.id());

        lifecycle.transition(AttackPhase::Planning, Some("building plan".into()))?;
        let plan = attack.plan(ctx).await?;

        lifecycle.transition(AttackPhase::Preparing, Some("preparing payloads".into()))?;
        let payloads = select_payloads(attack.as_ref(), &plan, ctx);
        let work_items = build_work_items(&self.mutator, &plan, &payloads, ctx)?;

        lifecycle.transition(AttackPhase::Executing, None)?;

        let attempts = if work_items.is_empty() {
            Vec::new()
        } else {
            self.execute_work_pool(attack.clone(), ctx, work_items, attempt_tx)
                .await?
        };

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

    async fn execute_work_pool(
        &self,
        attack: Arc<dyn Attack>,
        ctx: &AttackContext,
        work_items: Vec<WorkItem>,
        attempt_tx: Option<mpsc::Sender<AttemptStreamItem>>,
    ) -> AttackResult<Vec<PayloadAttempt>> {
        let concurrency = ctx.budget.concurrent_limit();
        let ctx = Arc::new(ctx.clone());
        let transport = self.transport.clone();
        let mut join_set = JoinSet::new();
        let mut items = work_items.into_iter();
        let mut indexed = Vec::new();

        loop {
            while join_set.len() < concurrency {
                let Some(item) = items.next() else {
                    break;
                };
                let delay_ms = ctx.budget.inter_request_delay_ms;
                if delay_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                let attack = attack.clone();
                let ctx = ctx.clone();
                let transport = transport.clone();

                join_set.spawn(async move {
                    run_work_item(&transport, attack.as_ref(), &ctx, item).await
                });
            }

            if join_set.is_empty() {
                break;
            }

            let (seq, attempt) = join_set
                .join_next()
                .await
                .ok_or_else(|| AttackError::invalid_state("attack worker pool ended unexpectedly"))?
                .map_err(|err| AttackError::invalid_state(format!("attack worker failed: {err}")))?
                .map_err(|err| AttackError::invalid_state(format!("attack attempt failed: {err}")))?;

            if let Some(ref tx) = attempt_tx {
                let _ = tx.send((seq, attempt.clone())).await;
            }
            indexed.push((seq, attempt));
        }

        indexed.sort_by_key(|(seq, _)| *seq);
        Ok(indexed.into_iter().map(|(_, attempt)| attempt).collect())
    }
}

struct WorkItem {
    seq: usize,
    payload: AttackPayload,
    content: String,
    mutators: Vec<MutatorKind>,
}

fn build_work_items(
    mutator: &PayloadMutator,
    plan: &crate::types::AttackPlan,
    payloads: &[AttackPayload],
    ctx: &AttackContext,
) -> AttackResult<Vec<WorkItem>> {
    let mut work_items = Vec::new();
    let mut seq = 0usize;
    let allowed_mutators = resolve_mutators(plan, ctx);

    for payload in payloads {
        if work_items.len() >= ctx.budget.max_payloads {
            break;
        }

        let variants = mutator.expand(&payload.content, &allowed_mutators)?;
        for (content, mutators) in variants {
            if work_items.len() >= ctx.budget.max_payloads {
                break;
            }
            work_items.push(WorkItem {
                seq,
                payload: payload.clone(),
                content: crate::attacks::preserve_canary_in_mutated(&content, payload),
                mutators,
            });
            seq += 1;
        }
    }

    Ok(work_items)
}

/// Category plan mutators (DB override or built-in), optionally filtered by allowlist.
fn resolve_mutators(
    plan: &crate::types::AttackPlan,
    ctx: &AttackContext,
) -> Vec<crate::payload::MutatorKind> {
    let base = ctx
        .mutator_plan_override
        .as_ref()
        .unwrap_or(&plan.mutators);
    match &ctx.enabled_mutators {
        None => base.clone(),
        Some(enabled) if enabled.is_empty() => Vec::new(),
        Some(enabled) => base
            .iter()
            .copied()
            .filter(|kind| enabled.contains(kind))
            .collect(),
    }
}

async fn run_work_item<T: TargetTransport>(
    transport: &T,
    attack: &dyn Attack,
    ctx: &AttackContext,
    item: WorkItem,
) -> AttackResult<(usize, PayloadAttempt)> {
    let runner = PayloadRunner::new(transport);
    let started = std::time::Instant::now();
    let response = match runner.execute(ctx, &item.payload, &item.content).await {
        Ok(response) => response,
        Err(err) => {
            // Soft-fail: keep sibling probes instead of aborting the whole pool.
            let body = err.to_string();
            let duration_ms = started.elapsed().as_millis() as u64;
            let attempt = PayloadAttempt {
                payload_id: item.payload.id.clone(),
                payload_name: item.payload.name.clone(),
                mutated_content: item.content,
                mutators_applied: item.mutators,
                response: AttackResponse {
                    status: 0,
                    headers: HashMap::new(),
                    body: body.clone(),
                    duration_ms,
                    normalized: NormalizedResponse {
                        content: String::new(),
                        raw_response: body.clone(),
                        status_code: None,
                        headers: HashMap::new(),
                        metadata: HashMap::from([
                            ("error".into(), "transport".into()),
                            ("harness".into(), "soft_fail".into()),
                        ]),
                    },
                },
                // `AttackError::Transport` already prefixes "transport error:".
                evaluation: AttackEvaluation::negative(body),
            };
            return Ok((item.seq, attempt));
        }
    };
    let evaluation = match attack.evaluate(ctx, &item.payload, &response).await {
        Ok(evaluation) => {
            crate::attacks::merge_canary_evaluation(&item.payload, &response, evaluation)
        }
        Err(err) => AttackEvaluation::negative(format!("evaluation error: {err}")),
    };

    let attempt = PayloadAttempt {
        payload_id: item.payload.id.clone(),
        payload_name: item.payload.name.clone(),
        mutated_content: item.content,
        mutators_applied: item.mutators,
        response,
        evaluation,
    };

    Ok((item.seq, attempt))
}

fn select_payloads(
    attack: &dyn Attack,
    plan: &crate::types::AttackPlan,
    ctx: &AttackContext,
) -> Vec<AttackPayload> {
    let category = attack.category();
    let mut payloads = if let Some(map) = &ctx.generated_payloads {
        if let Some(generated) = map.get(&category) {
            let filtered = filter_payload_list(generated, plan);
            if !filtered.is_empty() {
                filtered
                    .into_iter()
                    .take(ctx.budget.max_payloads)
                    .collect()
            } else {
                filter_defaults(attack, plan, ctx)
            }
        } else {
            filter_defaults(attack, plan, ctx)
        }
    } else {
        filter_defaults(attack, plan, ctx)
    };

    for payload in &mut payloads {
        crate::attacks::stamp_payload_canary(payload);
    }
    payloads
}

fn filter_defaults(
    attack: &dyn Attack,
    plan: &crate::types::AttackPlan,
    ctx: &AttackContext,
) -> Vec<AttackPayload> {
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
    use crate::types::{AttackBudget, AttackTarget, DEFAULT_ATTACK_CONCURRENCY};

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

    #[tokio::test]
    async fn pooled_execution_streams_attempts_in_order() {
        let transport = MockTransport::ok(r#"{"choices":[{"message":{"content":"ok"}}]}"#);
        let executor = AttackExecutor::new(AttackRegistry::with_builtins(), transport);
        let mut budget = AttackBudget::default();
        budget.max_payloads = 4;
        budget.max_concurrent_requests = 2;
        let mut ctx = AttackContext::new(
            "scan-1",
            "probe-pi",
            AttackTarget::llm_api("https://api.example.com/v1/chat/completions"),
        );
        ctx.budget = budget;

        let (tx, mut rx) = mpsc::channel(8);
        let exec = executor.execute_category_streaming(AttackCategory::PromptInjection, &ctx, tx);
        let mut streamed = Vec::new();
        let drain = async {
            while let Some((seq, attempt)) = rx.recv().await {
                streamed.push((seq, attempt.payload_id));
            }
        };

        let (result, ()) = tokio::join!(exec, drain);
        let result = result.unwrap();

        assert!(!result.attempts.is_empty());
        assert_eq!(streamed.len(), result.attempts.len());
        let mut seqs: Vec<_> = streamed.iter().map(|(seq, _)| *seq).collect();
        seqs.sort_unstable();
        assert_eq!(seqs, (0..result.attempts.len()).collect::<Vec<_>>());
        assert_eq!(DEFAULT_ATTACK_CONCURRENCY, 10);
    }
}
