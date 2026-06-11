use std::sync::Arc;

use time::OffsetDateTime;
use tracing::{info, instrument};

use crate::category::AttackCategory;
use crate::error::AttackResult;
use crate::executor::AttackExecutor;
use crate::transport::TargetTransport;
use crate::types::{AttackContext, OrchestrationReport};

/// Configuration for multi-attack orchestration.
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    pub categories: Vec<AttackCategory>,
    pub stop_on_first_critical: bool,
    pub concurrency: usize,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            categories: AttackCategory::all().to_vec(),
            stop_on_first_critical: false,
            concurrency: 1,
        }
    }
}

impl OrchestratorConfig {
    pub fn single(category: AttackCategory) -> Self {
        Self {
            categories: vec![category],
            ..Default::default()
        }
    }
}

/// Orchestrates multiple attacks against one target context.
pub struct AttackOrchestrator<T: TargetTransport> {
    executor: Arc<AttackExecutor<T>>,
    config: OrchestratorConfig,
}

impl<T: TargetTransport> AttackOrchestrator<T> {
    pub fn new(executor: AttackExecutor<T>, config: OrchestratorConfig) -> Self {
        Self {
            executor: Arc::new(executor),
            config,
        }
    }

    /// Run configured attack categories sequentially.
    #[instrument(skip(self, ctx), fields(scan_id = %ctx.scan_id))]
    pub async fn run(&self, ctx: &AttackContext) -> AttackResult<OrchestrationReport> {
        let started_at = OffsetDateTime::now_utc();
        let mut results = Vec::new();
        let mut findings_count = 0usize;

        for (idx, category) in self.config.categories.iter().enumerate() {
            let mut probe_ctx = ctx.clone();
            probe_ctx.probe_id = format!("{}-{}", ctx.probe_id, category.as_str());
            probe_ctx.metadata.insert(
                "category".into(),
                serde_json::Value::String(category.as_str().into()),
            );

            info!(category = %category, probe_id = %probe_ctx.probe_id, "orchestrating attack");

            match self.executor.execute_category(*category, &probe_ctx).await {
                Ok(result) => {
                    findings_count += result.successful_attempts().count();
                    let critical = result.attempts.iter().any(|a| {
                        a.evaluation.severity == Some(crate::types::FindingSeverity::Critical)
                    });
                    results.push(result);
                    if self.config.stop_on_first_critical && critical {
                        break;
                    }
                }
                Err(err) => {
                    results.push(crate::types::AttackExecutionResult {
                        attack_id: category.as_str().into(),
                        category: *category,
                        probe_id: probe_ctx.probe_id,
                        scan_id: ctx.scan_id.clone(),
                        phase: crate::lifecycle::AttackPhase::Failed,
                        attempts: vec![],
                        best: None,
                        started_at: OffsetDateTime::now_utc(),
                        completed_at: OffsetDateTime::now_utc(),
                        error: Some(err.to_string()),
                    });
                }
            }

            let _ = idx;
        }

        Ok(OrchestrationReport {
            scan_id: ctx.scan_id.clone(),
            results,
            findings_count,
            started_at,
            completed_at: OffsetDateTime::now_utc(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::AttackRegistry;
    use crate::transport::MockTransport;
    use crate::types::AttackTarget;

    #[tokio::test]
    async fn orchestrates_multiple_categories() {
        let transport = MockTransport::ok(r#"{"choices":[{"message":{"content":"ok"}}]}"#);
        let executor = AttackExecutor::new(AttackRegistry::with_builtins(), transport);
        let orchestrator = AttackOrchestrator::new(
            executor,
            OrchestratorConfig {
                categories: vec![
                    AttackCategory::PromptInjection,
                    AttackCategory::Jailbreak,
                ],
                ..Default::default()
            },
        );

        let ctx = AttackContext::new(
            "scan-1",
            "probe-main",
            AttackTarget::llm_api("https://api.example.com/v1/chat/completions"),
        );

        let report = orchestrator.run(&ctx).await.unwrap();
        assert_eq!(report.results.len(), 2);
    }
}
