use async_trait::async_trait;

use crate::error::AttackResult;
use crate::types::{AttackExecutionResult, FindingSeverity, OrchestrationReport};

/// Sink for persisting or forwarding attack results.
#[async_trait]
pub trait ResultSink: Send + Sync {
    async fn record_execution(&self, result: &AttackExecutionResult) -> AttackResult<()>;
    async fn record_orchestration(&self, report: &OrchestrationReport) -> AttackResult<()>;
}

/// In-memory result collector with optional sink delegation.
pub struct ResultCollector {
    executions: std::sync::Mutex<Vec<AttackExecutionResult>>,
    orchestrations: std::sync::Mutex<Vec<OrchestrationReport>>,
    sink: Option<Box<dyn ResultSink>>,
}

impl ResultCollector {
    pub fn new() -> Self {
        Self {
            executions: std::sync::Mutex::new(Vec::new()),
            orchestrations: std::sync::Mutex::new(Vec::new()),
            sink: None,
        }
    }

    pub fn with_sink(sink: Box<dyn ResultSink>) -> Self {
        Self {
            executions: std::sync::Mutex::new(Vec::new()),
            orchestrations: std::sync::Mutex::new(Vec::new()),
            sink: Some(sink),
        }
    }

    pub fn executions(&self) -> Vec<AttackExecutionResult> {
        self.executions.lock().unwrap().clone()
    }

    pub fn orchestrations(&self) -> Vec<OrchestrationReport> {
        self.orchestrations.lock().unwrap().clone()
    }

    pub fn successful_findings(&self) -> Vec<(String, FindingSeverity)> {
        self.executions
            .lock()
            .unwrap()
            .iter()
            .flat_map(|r| {
                r.attempts.iter().filter_map(|a| {
                    if a.evaluation.success {
                        Some((a.payload_name.clone(), a.evaluation.severity?))
                    } else {
                        None
                    }
                })
            })
            .collect()
    }

    pub async fn collect_execution(&self, result: AttackExecutionResult) -> AttackResult<()> {
        if let Some(sink) = &self.sink {
            sink.record_execution(&result).await?;
        }
        self.executions.lock().unwrap().push(result);
        Ok(())
    }

    pub async fn collect_orchestration(&self, report: OrchestrationReport) -> AttackResult<()> {
        if let Some(sink) = &self.sink {
            sink.record_orchestration(&report).await?;
        }
        for result in &report.results {
            self.executions.lock().unwrap().push(result.clone());
        }
        self.orchestrations.lock().unwrap().push(report);
        Ok(())
    }
}

impl Default for ResultCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::category::AttackCategory;
    use crate::lifecycle::AttackPhase;
    use crate::types::AttackEvaluation;
    use time::OffsetDateTime;

    #[tokio::test]
    async fn collects_successful_findings() {
        let collector = ResultCollector::new();
        collector
            .collect_execution(AttackExecutionResult {
                attack_id: "prompt_injection".into(),
                category: AttackCategory::PromptInjection,
                probe_id: "p1".into(),
                scan_id: "s1".into(),
                phase: AttackPhase::Completed,
                attempts: vec![crate::types::PayloadAttempt {
                    payload_id: "pi-1".into(),
                    payload_name: "direct override".into(),
                    mutated_content: "ignore".into(),
                    mutators_applied: vec![],
                    response: crate::types::AttackResponse {
                        status: 200,
                        headers: Default::default(),
                        body: "leaked".into(),
                        duration_ms: 1,
                        normalized: promptlab_harness::NormalizedResponse::from_http(
                            200,
                            "leaked".into(),
                            "mock",
                        ),
                    },
                    evaluation: AttackEvaluation::positive(
                        FindingSeverity::High,
                        0.9,
                        "injection succeeded",
                        vec!["override".into()],
                    ),
                }],
                best: None,
                started_at: OffsetDateTime::now_utc(),
                completed_at: OffsetDateTime::now_utc(),
                error: None,
            })
            .await
            .unwrap();

        assert_eq!(collector.successful_findings().len(), 1);
    }
}
