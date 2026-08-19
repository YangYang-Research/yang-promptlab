//! LLM-backed remediation recommendations from attack scan findings.

use std::collections::HashMap;

use promptlab_inference::PromptRegistry;
use promptlab_planner::{PlannerError, PlannerLlm, PlannerResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingSummaryInput {
    pub title: String,
    pub severity: String,
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackResultsSummary {
    pub scan_status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_url: Option<String>,
    /// Status counts for all attack scans on the same target.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub target_scan_status_counts: HashMap<String, usize>,
    pub total_findings: usize,
    pub severity_counts: HashMap<String, usize>,
    #[serde(default)]
    pub attack_categories: Vec<String>,
    pub findings: Vec<FindingSummaryInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackRecommendation {
    pub title: String,
    pub description: String,
    pub priority: String,
    /// Optional UI action: `retry_scan` | `start_attack` | `new_scan`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackRecommendationsBundle {
    pub overview: String,
    pub recommendations: Vec<AttackRecommendation>,
}

#[derive(Debug, Deserialize)]
struct LlmRecommendationsResponse {
    overview: String,
    recommendations: Vec<LlmRecommendationItem>,
}

#[derive(Debug, Deserialize)]
struct LlmRecommendationItem {
    title: String,
    description: String,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    action: Option<String>,
}

pub fn build_attack_results_summary(
    scan_status: &str,
    attack_categories: &[String],
    findings: &[FindingSummaryInput],
) -> AttackResultsSummary {
    let mut severity_counts = HashMap::new();
    for finding in findings {
        *severity_counts
            .entry(finding.severity.to_ascii_lowercase())
            .or_insert(0) += 1;
    }

    AttackResultsSummary {
        scan_status: scan_status.into(),
        scan_name: None,
        target_name: None,
        target_url: None,
        target_scan_status_counts: HashMap::new(),
        total_findings: findings.len(),
        severity_counts,
        attack_categories: attack_categories.to_vec(),
        findings: findings.to_vec(),
    }
}

pub async fn generate_attack_recommendations_with_llm(
    summary: &AttackResultsSummary,
    llm: &dyn PlannerLlm,
) -> PlannerResult<AttackRecommendationsBundle> {
    let summary_json = serde_json::to_string(summary)
        .map_err(|e| PlannerError::Llm(format!("failed to serialize findings summary: {e}")))?;
    let prompt = PromptRegistry::attack_results_recommend_user(&summary_json);
    let raw = llm.complete(&prompt).await?;
    parse_attack_recommendations(&raw)
}

/// Input for per-finding remediation recommendations (Finding Details).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingRemediationInput {
    pub finding_id: String,
    pub title: String,
    pub severity: String,
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<serde_json::Value>,
}

/// Generate fix-oriented recommendations for a single finding using the finding-specific prompt.
pub async fn generate_finding_recommendations_with_llm(
    finding: &FindingRemediationInput,
    llm: &dyn PlannerLlm,
) -> PlannerResult<AttackRecommendationsBundle> {
    let finding_json = serde_json::to_string(finding)
        .map_err(|e| PlannerError::Llm(format!("failed to serialize finding: {e}")))?;
    let prompt = PromptRegistry::finding_remediation_recommend_user(&finding_json);
    let raw = llm.complete(&prompt).await?;
    let mut bundle = parse_attack_recommendations(&raw)?;
    // Finding remediation never carries scan operational CTAs.
    for item in &mut bundle.recommendations {
        item.action = None;
    }
    Ok(bundle)
}

pub fn parse_attack_recommendations(raw: &str) -> PlannerResult<AttackRecommendationsBundle> {
    let json_str = extract_json_object(raw)?;
    let parsed: LlmRecommendationsResponse = serde_json::from_str(&json_str)
        .map_err(|e| PlannerError::Llm(format!("invalid recommendations JSON: {e}")))?;

    let overview = parsed.overview.trim();
    if overview.is_empty() {
        return Err(PlannerError::Llm(
            "recommendations overview must not be empty".into(),
        ));
    }

    if parsed.recommendations.is_empty() {
        return Err(PlannerError::Llm(
            "recommendations array must not be empty".into(),
        ));
    }

    let mut out = Vec::with_capacity(parsed.recommendations.len());
    for item in parsed.recommendations {
        let title = item.title.trim();
        let description = item.description.trim();
        if title.is_empty() || description.is_empty() {
            return Err(PlannerError::Llm(
                "each recommendation needs non-empty title and description".into(),
            ));
        }
        out.push(AttackRecommendation {
            title: title.into(),
            description: description.into(),
            priority: normalize_priority(item.priority.as_deref().unwrap_or("medium")),
            action: normalize_action(item.action.as_deref()),
        });
    }

    Ok(AttackRecommendationsBundle {
        overview: overview.into(),
        recommendations: out,
    })
}

/// Whether this scan status should surface a Retry Scan / Start Attack recommendation.
pub fn is_retryable_scan_status(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "failed" | "cancelled" | "canceled" | "stopped" | "error"
    )
}

/// Sort agent operational CTAs: Retry Scan first, ReScan last. Never inject copy.
/// Correct mis-tagged actions from scan_status (keep agent title/description).
pub fn ensure_rescan_action_recommendation(
    summary: &AttackResultsSummary,
    mut bundle: AttackRecommendationsBundle,
) -> AttackRecommendationsBundle {
    let retryable = is_retryable_scan_status(&summary.scan_status);
    let completed = summary.scan_status.trim().eq_ignore_ascii_case("completed");

    for rec in &mut bundle.recommendations {
        match rec.action.as_deref() {
            Some("retry_scan" | "start_attack") if completed => {
                rec.action = Some("new_scan".into());
                rec.priority = "info".into();
                if title_looks_like_retry_scan(&rec.title) {
                    rec.title = "ReScan".into();
                }
            }
            Some("new_scan" | "rescan") if retryable => {
                rec.action = Some("retry_scan".into());
                rec.priority = "high".into();
                if title_looks_like_rescan(&rec.title) {
                    rec.title = "Retry Scan".into();
                }
            }
            Some("new_scan" | "rescan") if completed => {
                rec.priority = "info".into();
            }
            Some("retry_scan" | "start_attack") if retryable => {
                rec.priority = "high".into();
            }
            _ => {}
        }
    }

    order_operational(bundle)
}

fn title_looks_like_retry_scan(title: &str) -> bool {
    title.to_ascii_lowercase().contains("retry")
}

fn title_looks_like_rescan(title: &str) -> bool {
    let t = title.to_ascii_lowercase();
    t.contains("rescan") || t.contains("re-scan") || t.contains("new scan")
}

pub fn ensure_failed_scan_action_recommendation(
    summary: &AttackResultsSummary,
    bundle: AttackRecommendationsBundle,
) -> AttackRecommendationsBundle {
    ensure_rescan_action_recommendation(summary, bundle)
}

fn order_operational(mut bundle: AttackRecommendationsBundle) -> AttackRecommendationsBundle {
    bundle.recommendations.sort_by_key(|r| match r.action.as_deref() {
        Some("retry_scan" | "start_attack") => 0,
        Some("new_scan" | "rescan") => 2,
        _ => 1,
    });
    bundle
}

fn normalize_action(raw: Option<&str>) -> Option<String> {
    match raw?.trim().to_ascii_lowercase().as_str() {
        "retry_scan" | "retry" | "retest" => Some("retry_scan".into()),
        "start_attack" | "start" | "attack" => Some("start_attack".into()),
        "new_scan" | "rescan" | "re-scan" => Some("new_scan".into()),
        _ => None,
    }
}

fn normalize_priority(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "critical" => "critical",
        "high" => "high",
        "medium" => "medium",
        "low" => "low",
        _ => "info",
    }
    .into()
}

fn extract_json_object(raw: &str) -> PlannerResult<String> {
    let trimmed = raw.trim();
    let start = trimmed
        .find('{')
        .ok_or_else(|| PlannerError::Llm("no JSON object in LLM response".into()))?;
    let slice = &trimmed[start..];

    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;

    for (idx, ch) in slice.char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            if ch == '\\' {
                escape = true;
                continue;
            }
            if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(slice[..=idx].to_string());
                }
            }
            _ => {}
        }
    }

    Err(PlannerError::Llm(
        "truncated or unterminated JSON in LLM response".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_summary_counts_severities() {
        let findings = vec![
            FindingSummaryInput {
                title: "Injection".into(),
                severity: "critical".into(),
                category: "prompt_injection".into(),
                description: None,
            },
            FindingSummaryInput {
                title: "Leak".into(),
                severity: "high".into(),
                category: "rag_leakage".into(),
                description: None,
            },
        ];
        let summary = build_attack_results_summary("completed", &["prompt_injection".into()], &findings);
        assert_eq!(summary.total_findings, 2);
        assert_eq!(summary.severity_counts.get("critical"), Some(&1));
        assert_eq!(summary.severity_counts.get("high"), Some(&1));
    }

    #[test]
    fn parse_recommendations_from_json() {
        let raw = r#"{"overview":"Scan found critical injection risk.","recommendations":[{"title":"Guardrails","description":"Add input filters.","priority":"critical"}]}"#;
        let bundle = parse_attack_recommendations(raw).unwrap();
        assert_eq!(bundle.overview, "Scan found critical injection risk.");
        assert_eq!(bundle.recommendations.len(), 1);
        assert_eq!(bundle.recommendations[0].title, "Guardrails");
        assert_eq!(bundle.recommendations[0].priority, "critical");
        assert_eq!(bundle.recommendations[0].action, None);
    }

    #[test]
    fn ensure_does_not_inject_retry_when_agent_omits_it() {
        let summary = build_attack_results_summary("failed", &[], &[]);
        let bundle = AttackRecommendationsBundle {
            overview: "Scan failed before completion.".into(),
            recommendations: vec![
                AttackRecommendation {
                    title: "Check auth".into(),
                    description: "Verify credentials.".into(),
                    priority: "high".into(),
                    action: None,
                },
                AttackRecommendation {
                    title: "Tighten rate limits".into(),
                    description: "Avoid throttling.".into(),
                    priority: "medium".into(),
                    action: None,
                },
            ],
        };
        let out = ensure_rescan_action_recommendation(&summary, bundle);
        assert_eq!(out.recommendations.len(), 2);
        assert!(out.recommendations.iter().all(|r| r.action.is_none()));
    }

    #[test]
    fn ensure_keeps_agent_copy_and_orders_rescan_last() {
        let summary = build_attack_results_summary("completed", &[], &[]);
        let agent_desc = "After remediating injection, run a new Agent Scan on 127.0.0.1.";
        let bundle = AttackRecommendationsBundle {
            overview: "Findings remain.".into(),
            recommendations: vec![
                AttackRecommendation {
                    title: "ReScan".into(),
                    description: agent_desc.into(),
                    priority: "info".into(),
                    action: Some("new_scan".into()),
                },
                AttackRecommendation {
                    title: "Add output filters".into(),
                    description: "Block leaked system prompts.".into(),
                    priority: "critical".into(),
                    action: None,
                },
            ],
        };
        let out = ensure_rescan_action_recommendation(&summary, bundle);
        assert_eq!(out.recommendations.len(), 2);
        assert_eq!(out.recommendations[0].title, "Add output filters");
        let last = out.recommendations.last().unwrap();
        assert_eq!(last.title, "ReScan");
        assert_eq!(last.action.as_deref(), Some("new_scan"));
        assert_eq!(last.priority, "info");
        assert_eq!(last.description, agent_desc);
    }

    #[test]
    fn ensure_coerces_retry_after_remediation_to_rescan_on_completed() {
        let summary = build_attack_results_summary("completed", &[], &[]);
        let agent_desc = "Retry the Agent Scan after implementing the recommended remediations \
             to ensure the vulnerabilities have been addressed.";
        let bundle = AttackRecommendationsBundle {
            overview: "Findings remain.".into(),
            recommendations: vec![AttackRecommendation {
                title: "Retry Scan After Remediation".into(),
                description: agent_desc.into(),
                priority: "high".into(),
                action: Some("retry_scan".into()),
            }],
        };
        let out = ensure_rescan_action_recommendation(&summary, bundle);
        assert_eq!(out.recommendations[0].title, "ReScan");
        assert_eq!(out.recommendations[0].action.as_deref(), Some("new_scan"));
        assert_eq!(out.recommendations[0].priority, "info");
        assert_eq!(out.recommendations[0].description, agent_desc);
    }

    #[test]
    fn ensure_keeps_agent_retry_copy() {
        let summary = build_attack_results_summary("failed", &[], &[]);
        let agent_desc = "Auth expired mid-run on 127.0.0.1; Retry Scan after rotating secrets.";
        let bundle = AttackRecommendationsBundle {
            overview: "Scan failed.".into(),
            recommendations: vec![
                AttackRecommendation {
                    title: "Rotate credentials".into(),
                    description: "Fix the 401 before continuing.".into(),
                    priority: "high".into(),
                    action: None,
                },
                AttackRecommendation {
                    title: "Retry Scan".into(),
                    description: agent_desc.into(),
                    priority: "high".into(),
                    action: Some("retry_scan".into()),
                },
            ],
        };
        let out = ensure_rescan_action_recommendation(&summary, bundle);
        assert_eq!(out.recommendations[0].title, "Retry Scan");
        assert_eq!(out.recommendations[0].action.as_deref(), Some("retry_scan"));
        assert_eq!(out.recommendations[0].description, agent_desc);
        assert_eq!(out.recommendations[1].title, "Rotate credentials");
    }
}
