use aisec_attack::AttackCategory;
use aisec_planner::{AttackPlan, FingerprintResult};
use tracing::{debug, info};

use crate::error::AgentResult;
use crate::host::AgentHost;
use crate::plan::intersect_categories;
use crate::retry::{generator_mode_for_retry, should_retry};
use crate::types::{
    AgentConfig, AgentPhase, AgentScanResult, AgentStopReason, CategoryAgentResult, PhaseRecord,
};

/// Run the agent loop for a single attack category on one endpoint.
pub async fn run_category_episode<H: AgentHost + ?Sized>(
    host: &mut H,
    config: &AgentConfig,
    plan: &AttackPlan,
    category: AttackCategory,
) -> AgentResult<CategoryAgentResult> {
    let mut phases = Vec::new();
    let mut attempt = 0u32;
    let mut retries = 0u32;
    let mut findings = 0u32;
    let mut vulnerable = false;
    let mut stop_reason = AgentStopReason::CategoryComplete;

    loop {
        if host.is_cancelled().await {
            stop_reason = AgentStopReason::Cancelled;
            break;
        }

        attempt += 1;
        let retry = attempt.saturating_sub(1);
        let generator_mode = generator_mode_for_retry(config, retry);

        record_phase(
            host,
            &mut phases,
            AgentPhase::Generate,
            &format!("mode={:?}", generator_mode),
            attempt,
            retry,
        )
        .await;

        let category_plan = single_category_plan(plan, category);
        let payloads = host
            .generate_payloads(&category_plan, category, generator_mode)
            .await?;

        record_phase(
            host,
            &mut phases,
            AgentPhase::Attack,
            &format!("{} payloads", payloads.stats.payload_count),
            attempt,
            retry,
        )
        .await;

        let mut execution = host.execute_attack(category, &payloads).await?;

        record_phase(
            host,
            &mut phases,
            AgentPhase::Judge,
            &format!("{} attempts", execution.attempts),
            attempt,
            retry,
        )
        .await;

        execution = host.evaluate_attack(category, &execution).await?;
        findings += execution.verdicts.iter().filter(|v| v.vulnerable).count() as u32;
        vulnerable = execution.any_vulnerable();

        if vulnerable {
            stop_reason = AgentStopReason::VulnerabilityFound;
            info!(category = %category.as_str(), attempt, "agent found vulnerability");
            break;
        }

        if !should_retry(false, attempt, config) {
            stop_reason = AgentStopReason::MaxAttemptsReached;
            debug!(
                category = %category.as_str(),
                attempt,
                "agent exhausted retry budget"
            );
            break;
        }

        retries += 1;
        record_phase(
            host,
            &mut phases,
            AgentPhase::Retry,
            &format!("escalate to {:?}", generator_mode_for_retry(config, retry + 1)),
            attempt,
            retry + 1,
        )
        .await;
    }

    record_phase(
        host,
        &mut phases,
        AgentPhase::Complete,
        stop_reason_label(stop_reason),
        attempt,
        retries,
    )
    .await;

    Ok(CategoryAgentResult {
        category,
        attempts: attempt,
        retries,
        vulnerable,
        phases,
        stop_reason,
        findings,
    })
}

/// Run the full agent loop for one endpoint across all planned categories.
pub async fn run_endpoint_agent<H: AgentHost + ?Sized>(
    host: &mut H,
    config: &AgentConfig,
    endpoint_id: &str,
    url: &str,
    allowed_categories: &[AttackCategory],
) -> AgentResult<AgentScanResult> {
    record_phase_standalone(host, AgentPhase::Fingerprint, url, 0, 0).await;
    let report = host.load_fingerprint(endpoint_id, url).await?;
    let fingerprint = FingerprintResult::single(endpoint_id, url, report);

    record_phase_standalone(host, AgentPhase::Plan, "generating attack plan", 0, 0).await;
    let raw_plan = host.plan(&fingerprint).await?;
    let plan = intersect_categories(&raw_plan, allowed_categories);

    let mut category_results = Vec::new();
    let mut total_attempts = 0u32;
    let mut total_retries = 0u32;
    let mut findings = 0u32;

    for category in &plan.categories {
        if host.is_cancelled().await {
            break;
        }
        let result = run_category_episode(host, config, &plan, *category).await?;
        total_attempts += result.attempts;
        total_retries += result.retries;
        findings += result.findings;
        category_results.push(result);
    }

    let summary = format!(
        "agent scan · {} categories · {} findings · {} attempts ({} retries)",
        category_results.len(),
        findings,
        total_attempts,
        total_retries
    );

    Ok(AgentScanResult {
        category_results,
        total_attempts,
        total_retries,
        findings,
        summary,
    })
}

fn single_category_plan(plan: &AttackPlan, category: AttackCategory) -> AttackPlan {
    AttackPlan {
        categories: vec![category],
        ..plan.clone()
    }
}

fn stop_reason_label(reason: AgentStopReason) -> &'static str {
    match reason {
        AgentStopReason::VulnerabilityFound => "vulnerability_found",
        AgentStopReason::MaxAttemptsReached => "max_attempts",
        AgentStopReason::CategoryComplete => "category_complete",
        AgentStopReason::Cancelled => "cancelled",
    }
}

async fn record_phase<H: AgentHost + ?Sized>(
    host: &mut H,
    phases: &mut Vec<PhaseRecord>,
    phase: AgentPhase,
    detail: &str,
    attempt: u32,
    retry: u32,
) {
    host.on_phase(phase, detail, attempt, retry).await;
    phases.push(PhaseRecord {
        phase,
        detail: detail.into(),
        attempt,
        retry,
    });
}

async fn record_phase_standalone<H: AgentHost + ?Sized>(
    host: &mut H,
    phase: AgentPhase,
    detail: &str,
    attempt: u32,
    retry: u32,
) {
    host.on_phase(phase, detail, attempt, retry).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AttackExecutionSummary;
    use aisec_fingerprint::{FingerprintReport, PlatformProfile, StackFingerprintReport};
    use time::OffsetDateTime;
    use aisec_generator::{GeneratorMode, GeneratorStats, PromptPayloads};
    use aisec_planner::{AttackPlan, PlannerMode};
    use async_trait::async_trait;
    use std::collections::HashMap;

    struct MockHost {
        cancelled: bool,
        vulnerable_on_attempt: u32,
        calls: u32,
    }

    #[async_trait]
    impl AgentHost for MockHost {
        async fn is_cancelled(&self) -> bool {
            self.cancelled
        }

        async fn load_fingerprint(
            &self,
            _endpoint_id: &str,
            _url: &str,
        ) -> AgentResult<StackFingerprintReport> {
            Ok(StackFingerprintReport {
                url: "https://test".into(),
                confidence: 0.5,
                technologies: vec![],
                agent_frameworks: vec![],
                ai_components: vec![],
                provider_report: FingerprintReport {
                    url: "https://test".into(),
                    matches: vec![],
                    primary: None,
                    analyzed_at: OffsetDateTime::now_utc(),
                },
                attack_recommendations: vec![],
                methods_used: vec![],
                platform_profile: PlatformProfile::default(),
                analyzed_at: OffsetDateTime::now_utc(),
            })
        }

        async fn on_phase(
            &mut self,
            _phase: AgentPhase,
            _detail: &str,
            _attempt: u32,
            _retry: u32,
        ) {
        }

        async fn plan(&mut self, _fingerprint: &FingerprintResult) -> AgentResult<AttackPlan> {
            Ok(AttackPlan {
                mode: PlannerMode::Deterministic,
                profile_id: "test".into(),
                categories: vec![AttackCategory::PromptInjection],
                disabled_tests: vec![],
                rationales: vec![],
                confidence: 1.0,
                summary: String::new(),
                llm_rationale: None,
            })
        }

        async fn generate_payloads(
            &mut self,
            _plan: &AttackPlan,
            category: AttackCategory,
            mode: GeneratorMode,
        ) -> AgentResult<PromptPayloads> {
            let _ = mode;
            Ok(PromptPayloads {
                mode: GeneratorMode::StaticPack,
                by_category: HashMap::from([(
                    category,
                    vec![aisec_attack::AttackPayload::new(
                        "p1",
                        "test",
                        category,
                        "probe",
                    )],
                )]),
                payload_ids: vec!["p1".into()],
                stats: GeneratorStats {
                    category_count: 1,
                    source_count: 1,
                    payload_count: 1,
                    variant_count: 1,
                },
                summary: String::new(),
                llm_note: None,
            })
        }

        async fn execute_attack(
            &mut self,
            category: AttackCategory,
            _payloads: &PromptPayloads,
        ) -> AgentResult<AttackExecutionSummary> {
            self.calls += 1;
            Ok(AttackExecutionSummary {
                category,
                attempts: 1,
                verdicts: vec![],
            })
        }

        async fn evaluate_attack(
            &mut self,
            category: AttackCategory,
            execution: &AttackExecutionSummary,
        ) -> AgentResult<AttackExecutionSummary> {
            let vulnerable = self.calls >= self.vulnerable_on_attempt;
            Ok(AttackExecutionSummary {
                category,
                attempts: execution.attempts,
                verdicts: vec![crate::types::AgentVerdict {
                    payload_id: "p1".into(),
                    payload_name: "test".into(),
                    vulnerable,
                    confidence: if vulnerable { 0.9 } else { 0.1 },
                    summary: if vulnerable {
                        "vulnerable".into()
                    } else {
                        "clean".into()
                    },
                }],
            })
        }
    }

    #[tokio::test]
    async fn retries_until_vulnerability() {
        let plan = AttackPlan {
            mode: PlannerMode::Deterministic,
            profile_id: "test".into(),
            categories: vec![AttackCategory::PromptInjection],
            disabled_tests: vec![],
            rationales: vec![],
            confidence: 1.0,
            summary: String::new(),
            llm_rationale: None,
        };
        let config = AgentConfig {
            max_attempts_per_category: 5,
            ..AgentConfig::default()
        };
        let mut host = MockHost {
            cancelled: false,
            vulnerable_on_attempt: 3,
            calls: 0,
        };
        let result = run_category_episode(&mut host, &config, &plan, AttackCategory::PromptInjection)
            .await
            .unwrap();
        assert!(result.vulnerable);
        assert_eq!(result.attempts, 3);
        assert_eq!(result.retries, 2);
        assert_eq!(result.stop_reason, AgentStopReason::VulnerabilityFound);
    }
}
