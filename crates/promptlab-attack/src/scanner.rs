//! Prompt Injection Scanner.
//!
//! Ties the attack framework to persistent storage to deliver an end-to-end
//! prompt-injection scan:
//!
//! 1. **Load payload library** — the built-in prompt-injection payloads (and
//!    their mutations) from the attack registry.
//! 2. **Send payloads** — through [`HarnessTransport`] → [`HarnessFactory`].
//! 3. **Capture responses** — the actual target responses, evaluated for
//!    injection indicators.
//! 4. **Store findings** — successful injections are persisted to SQLite via
//!    `promptlab-storage` (along with every probe as an `attack_result`).
//!
//! This module is only compiled with the `storage` feature.

use promptlab_storage::{
    AttackResultRepository, CreateAttackResult, CreateFinding, Database, FindingRepository,
};
use tracing::{info, instrument};

use crate::category::AttackCategory;
use crate::error::AttackResult;
use crate::executor::AttackExecutor;
use crate::registry::AttackRegistry;
use crate::transport::HarnessTransport;
use crate::types::{
    AttackBudget, AttackContext, AttackExecutionResult, AttackTarget, FindingSeverity, PayloadAttempt,
};

/// Storage identifiers that findings are persisted under.
///
/// The caller is responsible for creating the `project` and `scan` rows (so the
/// foreign keys resolve) and passing their ids here.
#[derive(Debug, Clone)]
pub struct ScanContext {
    pub scan_id: String,
    pub project_id: String,
    pub target_id: Option<String>,
}

impl ScanContext {
    pub fn new(scan_id: impl Into<String>, project_id: impl Into<String>) -> Self {
        Self {
            scan_id: scan_id.into(),
            project_id: project_id.into(),
            target_id: None,
        }
    }

    pub fn with_target(mut self, target_id: impl Into<String>) -> Self {
        self.target_id = Some(target_id.into());
        self
    }
}

/// Outcome of a prompt-injection scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanSummary {
    /// Number of payloads (including mutations) actually sent through the harness.
    pub payloads_sent: usize,
    /// Number of responses captured.
    pub responses_captured: usize,
    /// Number of findings persisted to storage.
    pub findings_stored: usize,
    /// Highest finding severity observed, if any.
    pub highest_severity: Option<FindingSeverity>,
}

/// End-to-end prompt-injection scanner backed by harness transport and SQLite storage.
pub struct PromptInjectionScanner {
    db: Database,
}

impl PromptInjectionScanner {
    /// Build a scanner that persists findings to `db`.
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Run the prompt-injection scan against `target`, persisting results.
    #[instrument(skip(self, target), fields(scan_id = %scan.scan_id, url = %target.url))]
    pub async fn scan(
        &self,
        target: AttackTarget,
        scan: &ScanContext,
        budget: AttackBudget,
    ) -> AttackResult<ScanSummary> {
        let transport = HarnessTransport::for_attack_target(&target)?;
        let executor = AttackExecutor::new(AttackRegistry::with_builtins(), transport);

        let mut ctx = AttackContext::new(
            scan.scan_id.clone(),
            format!("{}-prompt-injection", scan.scan_id),
            target,
        );
        ctx.budget = budget;
        ctx.target_id = scan.target_id.clone();

        let result = executor
            .execute_category(AttackCategory::PromptInjection, &ctx)
            .await?;

        let summary = self.persist(&result, scan).await?;

        info!(
            payloads_sent = summary.payloads_sent,
            findings_stored = summary.findings_stored,
            "prompt injection scan complete"
        );
        Ok(summary)
    }

    async fn persist(
        &self,
        result: &AttackExecutionResult,
        scan: &ScanContext,
    ) -> AttackResult<ScanSummary> {
        let repos = self.db.repositories();
        let mut findings_stored = 0usize;
        let mut highest: Option<FindingSeverity> = None;

        for attempt in &result.attempts {
            repos
                .attack_results()
                .create(CreateAttackResult {
                    scan_id: scan.scan_id.clone(),
                    payload_id: None,
                    target_id: scan.target_id.clone(),
                    probe_id: Some(result.probe_id.clone()),
                    success: attempt.evaluation.success,
                    response_json: serde_json::to_value(&attempt.response).ok(),
                    evaluated_json: serde_json::to_value(&attempt.evaluation).ok(),
                    duration_ms: Some(attempt.response.duration_ms as i64),
                })
                .await?;

            if attempt.evaluation.success {
                let severity = attempt.evaluation.severity.unwrap_or(FindingSeverity::Medium);
                repos
                    .findings()
                    .create(CreateFinding {
                        scan_id: scan.scan_id.clone(),
                        project_id: scan.project_id.clone(),
                        target_id: scan.target_id.clone(),
                        title: format!("Prompt injection: {}", attempt.payload_name),
                        severity: severity_str(severity).to_string(),
                        category: Some("prompt_injection".to_string()),
                        description: Some(attempt.evaluation.summary.clone()),
                        evidence_json: Some(evidence_json(attempt)),
                        status: None,
                    })
                    .await?;

                findings_stored += 1;
                highest = Some(highest.map_or(severity, |h| h.max(severity)));
            }
        }

        Ok(ScanSummary {
            payloads_sent: result.attempts.len(),
            responses_captured: result.attempts.len(),
            findings_stored,
            highest_severity: highest,
        })
    }
}

fn severity_str(severity: FindingSeverity) -> &'static str {
    match severity {
        FindingSeverity::Info => "info",
        FindingSeverity::Low => "low",
        FindingSeverity::Medium => "medium",
        FindingSeverity::High => "high",
        FindingSeverity::Critical => "critical",
    }
}

fn evidence_json(attempt: &PayloadAttempt) -> serde_json::Value {
    let excerpt: String = attempt.response.body.chars().take(800).collect();
    serde_json::json!({
        "payload_id": attempt.payload_id,
        "payload_name": attempt.payload_name,
        "sent_payload": attempt.mutated_content,
        "mutators_applied": attempt.mutators_applied,
        "indicators": attempt.evaluation.indicators,
        "confidence": attempt.evaluation.confidence,
        "response_status": attempt.response.status,
        "response_excerpt": excerpt,
        "normalized_content": attempt.response.normalized.content,
    })
}
