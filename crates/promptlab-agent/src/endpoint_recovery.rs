//! Endpoint failure recovery: pacing / backoff plans for attack-execution agents.
//!
//! Used by SequentialAttackExecutionAgent and AgenticAttackExecutionAgent when HTTP
//! transport fails, rate-limits, or the endpoint shows unhealthy latency.

use crate::attack_execution::AttackAttemptObservation;

pub const DEFAULT_ATTACK_CONCURRENCY: usize = 10;
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;
pub const MAX_ENDPOINT_RECOVERIES: u32 = 3;

/// Live request pacing applied by the host before the next attack attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointPacing {
    pub max_concurrent_requests: usize,
    pub inter_request_delay_ms: u64,
    pub timeout_ms: u64,
    /// Force one-in-flight request and wait for each response before the next.
    pub serial_wait: bool,
}

impl Default for EndpointPacing {
    fn default() -> Self {
        Self {
            max_concurrent_requests: DEFAULT_ATTACK_CONCURRENCY,
            inter_request_delay_ms: 0,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            serial_wait: false,
        }
    }
}

impl EndpointPacing {
    pub fn effective_concurrency(&self) -> usize {
        if self.serial_wait {
            1
        } else {
            self.max_concurrent_requests.max(1)
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "concurrency={} delay_ms={} timeout_ms={} serial={}",
            self.effective_concurrency(),
            self.inter_request_delay_ms,
            self.timeout_ms,
            self.serial_wait
        )
    }

    /// True when pacing still matches the host default (no recover / inherit applied).
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

/// Concrete recovery step to apply before retrying the attack.
#[derive(Debug, Clone)]
pub struct RecoveryPlan {
    pub pacing: EndpointPacing,
    pub wait_before_retry_ms: u64,
    pub notes: Vec<String>,
}

impl RecoveryPlan {
    pub fn summary(&self) -> String {
        let notes = if self.notes.is_empty() {
            String::new()
        } else {
            format!(" | {}", self.notes.join("; "))
        };
        format!(
            "pacing{{{}}} wait_ms={}{}",
            self.pacing.summary(),
            self.wait_before_retry_ms,
            notes
        )
    }
}

/// True when an attack-tool error is an internal/judge failure — not an endpoint
/// transport problem. Recover+re-attack would waste the whole batch.
pub fn error_is_endpoint_recoverable(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    if lower.contains("[internal]")
        || lower.contains("judge failed")
        || lower.contains("judgecoordinator")
        || lower.contains("all role workers failed")
        || lower.contains("ai runtime")
    {
        return false;
    }
    true
}

/// True when the last attack observation indicates the endpoint is unhealthy / throttling.
///
/// Partial success (some HTTP 200s + a minority of transport/5xx failures, or high latency
/// alone) does **not** force recover — that was causing costly re-attacks after usable probes.
/// High-confidence findings also skip recover: the category already succeeded for vuln goals.
pub fn observation_needs_recovery(obs: &AttackAttemptObservation) -> bool {
    if obs.high_confidence_vuln && obs.http_successes > 0 {
        return false;
    }
    if obs.endpoint_error.as_ref().is_some_and(|e| !e.trim().is_empty()) {
        return true;
    }
    if obs.rate_limited > 0 {
        return true;
    }
    if obs.endpoint_unhealthy {
        return true;
    }

    let hard_failures = obs.transport_errors.saturating_add(obs.server_errors);
    if obs.attempts > 0 && obs.http_successes == 0 {
        if hard_failures > 0 {
            return true;
        }
        // No usable HTTP successes and high latency → likely overload / hang.
        if obs.avg_latency_ms >= 10_000 || obs.max_latency_ms >= 20_000 {
            return true;
        }
        return false;
    }

    // Partial success: only recover when hard failures dominate successes.
    hard_failures > 0 && hard_failures >= obs.http_successes
}

/// Deterministic pacing adjustment from observation + recovery attempt index.
pub fn heuristic_recovery(
    obs: &AttackAttemptObservation,
    current: &EndpointPacing,
    recovery_index: u32,
) -> RecoveryPlan {
    let mut next = current.clone();
    let mut notes = Vec::new();
    let mut wait_ms = 1_000u64.saturating_mul(u64::from(recovery_index.saturating_add(1)));

    let err = obs
        .endpoint_error
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    let rate_signal = obs.rate_limited > 0
        || err.contains("429")
        || err.contains("rate")
        || err.contains("throttl")
        || obs.summary.to_ascii_lowercase().contains("429");
    let transport_signal = obs.transport_errors > 0
        || obs.server_errors > 0
        || err.contains("timeout")
        || err.contains("timed out")
        || err.contains("connection")
        || err.contains("transport");
    let latency_signal = obs.avg_latency_ms >= 5_000 || obs.max_latency_ms >= 15_000;

    // Total wipeout (no HTTP successes) or later recoveries → serialize immediately.
    let total_transport_wipeout = obs.attempts > 0
        && obs.http_successes == 0
        && (obs.transport_errors > 0 || obs.server_errors > 0 || transport_signal);

    if recovery_index >= 1 || current.serial_wait || total_transport_wipeout {
        next.serial_wait = true;
        next.max_concurrent_requests = 1;
        if total_transport_wipeout && recovery_index == 0 && !current.serial_wait {
            notes.push("serial: total transport/server wipeout on first recover".into());
        } else {
            notes.push("serial: one request, wait for response".into());
        }
    } else {
        let lowered = (current.effective_concurrency() / 2).max(1);
        next.max_concurrent_requests = lowered;
        if lowered == 1 {
            next.serial_wait = true;
            notes.push("drop to serial concurrency=1".into());
        } else {
            notes.push(format!("lower concurrency to {lowered}"));
        }
    }

    if rate_signal {
        next.inter_request_delay_ms = current
            .inter_request_delay_ms
            .saturating_add(2_000)
            .saturating_add(1_000 * u64::from(recovery_index))
            .min(30_000);
        wait_ms = wait_ms.saturating_mul(2).min(30_000);
        notes.push(format!(
            "rate-limit backoff delay_ms={}",
            next.inter_request_delay_ms
        ));
    } else if transport_signal {
        next.inter_request_delay_ms = current
            .inter_request_delay_ms
            .saturating_add(1_000)
            .saturating_add(500 * u64::from(recovery_index))
            .min(30_000);
        if total_transport_wipeout {
            // Full wipeout: longer timeouts only stall serial retries when the
            // endpoint is dead or hanging until the previous timeout fires.
            notes.push(format!(
                "transport wipeout keep timeout_ms={} delay_ms={}",
                next.timeout_ms, next.inter_request_delay_ms
            ));
        } else {
            next.timeout_ms = current
                .timeout_ms
                .saturating_mul(3)
                .saturating_div(2)
                .clamp(30_000, 120_000);
            notes.push(format!(
                "transport/server recovery timeout_ms={} delay_ms={}",
                next.timeout_ms, next.inter_request_delay_ms
            ));
        }
    } else if latency_signal {
        let extra = (obs.avg_latency_ms / 2).clamp(500, 10_000);
        next.inter_request_delay_ms = current
            .inter_request_delay_ms
            .saturating_add(extra)
            .min(30_000);
        next.serial_wait = true;
        next.max_concurrent_requests = 1;
        wait_ms = obs.avg_latency_ms.clamp(1_000, 15_000);
        notes.push(format!(
            "latency-tuned delay_ms={} (avg_latency_ms={})",
            next.inter_request_delay_ms, obs.avg_latency_ms
        ));
    } else {
        next.inter_request_delay_ms = current
            .inter_request_delay_ms
            .saturating_add(500)
            .min(30_000);
        notes.push(format!(
            "generic pacing delay_ms={}",
            next.inter_request_delay_ms
        ));
    }

    RecoveryPlan {
        pacing: next,
        wait_before_retry_ms: wait_ms,
        notes,
    }
}

/// Strong pacing seed from a prior-scan failure for this category.
///
/// Applied once at category start — does **not** consume a recovery slot.
/// Uses recovery_index=1 (serial) so known-bad endpoints do not start at mild concurrency=5.
/// Recover ReAct actions remain reserved for unhealthy observations in the current run.
pub fn seed_pacing_from_prior_failure(current: &EndpointPacing) -> RecoveryPlan {
    let mut plan = heuristic_recovery(
        &AttackAttemptObservation {
            endpoint_unhealthy: true,
            transport_errors: 1,
            attempts: 1,
            http_successes: 0,
            ..Default::default()
        },
        current,
        1,
    );
    plan.wait_before_retry_ms = 0;
    plan.notes.insert(
        0,
        "seeded from prior category failure (not counted as recover)".into(),
    );
    plan
}

/// True when this batch was slow or lossy enough that the *next* category should
/// inherit serial pacing — without Recover/re-attack on the current category.
pub fn batch_needs_degraded_pacing(
    attempts: u64,
    http_successes: u64,
    transport_errors: u64,
    server_errors: u64,
    rate_limited: u64,
    avg_latency_ms: u64,
) -> bool {
    if rate_limited > 0 {
        return true;
    }
    let hard = transport_errors.saturating_add(server_errors);
    if attempts == 0 {
        return false;
    }
    if hard == 0 {
        return avg_latency_ms >= 20_000;
    }
    (hard >= 3 && hard.saturating_mul(5) >= attempts)
        || hard >= http_successes
        || avg_latency_ms >= 20_000
}

/// Serial pacing for the next category after a strained batch. Does not consume a recover slot.
pub fn degrade_pacing_for_next_category(current: &EndpointPacing) -> EndpointPacing {
    seed_pacing_from_prior_failure(current).pacing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_forces_backoff_and_lower_concurrency() {
        let obs = AttackAttemptObservation {
            rate_limited: 2,
            attempts: 5,
            http_successes: 1,
            endpoint_unhealthy: true,
            ..Default::default()
        };
        let plan = heuristic_recovery(&obs, &EndpointPacing::default(), 0);
        assert_eq!(plan.pacing.effective_concurrency(), 5);
        assert!(plan.pacing.inter_request_delay_ms >= 2_000);
        assert!(plan.wait_before_retry_ms >= 2_000);
    }

    #[test]
    fn total_transport_wipeout_serializes_on_first_recover() {
        let obs = AttackAttemptObservation {
            attempts: 29,
            http_successes: 0,
            transport_errors: 29,
            endpoint_unhealthy: true,
            ..Default::default()
        };
        let plan = heuristic_recovery(&obs, &EndpointPacing::default(), 0);
        assert!(plan.pacing.serial_wait);
        assert_eq!(plan.pacing.effective_concurrency(), 1);
        // Wipeout must not inflate timeout — that stalls serial fail-fast retries.
        assert_eq!(plan.pacing.timeout_ms, DEFAULT_TIMEOUT_MS);
    }

    #[test]
    fn high_confidence_skips_recovery() {
        let obs = AttackAttemptObservation {
            attempts: 29,
            http_successes: 5,
            transport_errors: 24,
            high_confidence_vuln: true,
            endpoint_unhealthy: true,
            ..Default::default()
        };
        assert!(!observation_needs_recovery(&obs));
    }

    #[test]
    fn judge_internal_errors_are_not_endpoint_recoverable() {
        assert!(!error_is_endpoint_recoverable(
            "[INTERNAL] judge failed: JudgeCoordinatorAgent: all role workers failed — check AI runtime"
        ));
        assert!(!error_is_endpoint_recoverable(
            "JudgeCoordinatorAgent: all role workers failed"
        ));
        assert!(error_is_endpoint_recoverable(
            "transport error: connection reset"
        ));
    }

    #[test]
    fn second_recovery_is_serial() {
        let obs = AttackAttemptObservation {
            transport_errors: 1,
            endpoint_error: Some("connection reset".into()),
            endpoint_unhealthy: true,
            ..Default::default()
        };
        let plan = heuristic_recovery(&obs, &EndpointPacing::default(), 1);
        assert!(plan.pacing.serial_wait);
        assert_eq!(plan.pacing.effective_concurrency(), 1);
    }

    #[test]
    fn latency_tunes_inter_request_delay() {
        let obs = AttackAttemptObservation {
            attempts: 4,
            http_successes: 0,
            avg_latency_ms: 8_000,
            max_latency_ms: 12_000,
            endpoint_unhealthy: true,
            ..Default::default()
        };
        assert!(observation_needs_recovery(&obs));
        let plan = heuristic_recovery(&obs, &EndpointPacing::default(), 0);
        assert!(plan.pacing.serial_wait);
        assert!(plan.pacing.inter_request_delay_ms >= 4_000);
    }

    #[test]
    fn seed_pacing_does_not_wait() {
        let plan = seed_pacing_from_prior_failure(&EndpointPacing::default());
        assert_eq!(plan.wait_before_retry_ms, 0);
        assert!(plan.pacing.serial_wait);
        assert_eq!(plan.pacing.effective_concurrency(), 1);
        assert!(plan.notes.iter().any(|n| n.contains("not counted as recover")));
    }

    #[test]
    fn strained_batch_inherits_serial_without_recovery() {
        assert!(batch_needs_degraded_pacing(30, 23, 7, 0, 0, 70_000));
        assert!(batch_needs_degraded_pacing(30, 17, 13, 0, 0, 85_000));
        assert!(!batch_needs_degraded_pacing(12, 12, 0, 0, 0, 8_000));
        assert!(!batch_needs_degraded_pacing(30, 29, 1, 0, 0, 3_000));
        let next = degrade_pacing_for_next_category(&EndpointPacing::default());
        assert!(next.serial_wait);
        assert_eq!(next.effective_concurrency(), 1);
    }

    #[test]
    fn partial_success_does_not_need_recovery() {
        let obs = AttackAttemptObservation {
            attempts: 3,
            http_successes: 2,
            transport_errors: 1,
            avg_latency_ms: 25_000,
            max_latency_ms: 44_000,
            endpoint_unhealthy: false,
            ..Default::default()
        };
        assert!(!observation_needs_recovery(&obs));
    }

    #[test]
    fn failures_dominating_successes_need_recovery() {
        let obs = AttackAttemptObservation {
            attempts: 3,
            http_successes: 1,
            transport_errors: 2,
            endpoint_unhealthy: true,
            ..Default::default()
        };
        assert!(observation_needs_recovery(&obs));
    }

    #[test]
    fn default_pacing_detects_unmodified_state() {
        assert!(EndpointPacing::default().is_default());
        let mut escalated = EndpointPacing::default();
        escalated.serial_wait = true;
        escalated.max_concurrent_requests = 1;
        escalated.inter_request_delay_ms = 5_500;
        escalated.timeout_ms = 120_000;
        assert!(!escalated.is_default());
    }
}
