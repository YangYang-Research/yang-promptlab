//! Project health score (0–100) derived from finding severities.

use crate::models::Finding;

/// Severity risk weights — mirrors `Severity::risk_weight` in promptlab-report.
fn severity_risk_weight(severity: &str) -> u32 {
    match severity.to_ascii_lowercase().as_str() {
        "critical" => 16,
        "high" => 8,
        "medium" => 4,
        "low" => 2,
        _ => 1, // info / unknown
    }
}

/// Compute project health score 0–100 (higher is healthier).
///
/// Inverts the report risk-gauge normalization: `risk% = raw / (n × 16)`.
/// Empty findings → 100.
pub fn compute_health_score(findings: &[Finding]) -> i64 {
    if findings.is_empty() {
        return 100;
    }

    let mut raw_risk: u32 = 0;
    for finding in findings {
        raw_risk += severity_risk_weight(&finding.severity);
    }
    let max_risk = (findings.len() as u32).saturating_mul(16).max(1);
    let risk_pct = ((raw_risk as f64 / max_risk as f64) * 100.0).min(100.0);
    (100.0 - risk_pct).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;

    fn finding(severity: &str) -> Finding {
        let now = OffsetDateTime::now_utc();
        Finding {
            id: "f".into(),
            scan_id: "s".into(),
            project_id: "p".into(),
            target_id: None,
            title: "t".into(),
            severity: severity.into(),
            category: None,
            description: None,
            evidence_json: None,
            status: "open".into(),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn empty_findings_score_100() {
        assert_eq!(compute_health_score(&[]), 100);
    }

    #[test]
    fn all_critical_scores_0() {
        assert_eq!(
            compute_health_score(&[finding("critical"), finding("critical")]),
            0
        );
    }

    #[test]
    fn mixed_severities() {
        // raw = 16+8+1 = 25, max = 48 → risk ≈ 52% → score ≈ 48
        assert_eq!(
            compute_health_score(&[finding("critical"), finding("high"), finding("info")]),
            48
        );
    }
}
