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
    /// Optional UI action: `retry_scan` | `start_attack`.
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

/// Ensure failed/incomplete scans include one actionable Retry Scan recommendation
/// (in addition to up to 4 remediation items).
pub fn ensure_failed_scan_action_recommendation(
    summary: &AttackResultsSummary,
    mut bundle: AttackRecommendationsBundle,
) -> AttackRecommendationsBundle {
    if !is_retryable_scan_status(&summary.scan_status) {
        return bundle;
    }

    // Normalize LLM-produced retry-ish items into a typed action.
    for rec in &mut bundle.recommendations {
        if rec.action.is_none() && title_looks_like_retry(&rec.title) {
            rec.action = Some("retry_scan".into());
            rec.priority = "high".into();
        }
    }

    if bundle
        .recommendations
        .iter()
        .any(|r| matches!(r.action.as_deref(), Some("retry_scan" | "start_attack")))
    {
        return order_action_first(bundle);
    }

    let mut remediation: Vec<AttackRecommendation> = bundle
        .recommendations
        .into_iter()
        .filter(|r| r.action.is_none())
        .take(4)
        .collect();

    let mut recommendations = Vec::with_capacity(remediation.len() + 1);
    recommendations.push(failed_scan_action_recommendation(summary));
    recommendations.append(&mut remediation);

    AttackRecommendationsBundle {
        overview: bundle.overview,
        recommendations,
    }
}

fn failed_scan_action_recommendation(summary: &AttackResultsSummary) -> AttackRecommendation {
    let status = summary.scan_status.trim().to_ascii_lowercase();
    let target = summary
        .target_name
        .as_deref()
        .or(summary.target_url.as_deref())
        .unwrap_or("this target");
    AttackRecommendation {
        title: "Retry Scan".into(),
        description: format!(
            "This attack scan ended as `{status}` on {target}. Open the scan wizard to Retry Scan \
             (review plan/auth) or Start Attack immediately to re-run the assessment."
        ),
        priority: "high".into(),
        action: Some("retry_scan".into()),
    }
}

fn title_looks_like_retry(title: &str) -> bool {
    let t = title.to_ascii_lowercase();
    t.contains("retry") || t.contains("re-run") || t.contains("rerun") || t.contains("start attack")
}

fn order_action_first(mut bundle: AttackRecommendationsBundle) -> AttackRecommendationsBundle {
    bundle.recommendations.sort_by_key(|r| {
        if matches!(r.action.as_deref(), Some("retry_scan" | "start_attack")) {
            0
        } else {
            1
        }
    });
    bundle
}

fn normalize_action(raw: Option<&str>) -> Option<String> {
    match raw?.trim().to_ascii_lowercase().as_str() {
        "retry_scan" | "retry" | "retest" => Some("retry_scan".into()),
        "start_attack" | "start" | "attack" => Some("start_attack".into()),
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
    fn ensure_injects_retry_for_failed_scan() {
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
        let out = ensure_failed_scan_action_recommendation(&summary, bundle);
        assert_eq!(out.recommendations.len(), 3);
        assert_eq!(out.recommendations[0].action.as_deref(), Some("retry_scan"));
        assert_eq!(out.recommendations[0].title, "Retry Scan");
    }

    #[test]
    fn ensure_skips_completed_scan() {
        let summary = build_attack_results_summary("completed", &[], &[]);
        let bundle = AttackRecommendationsBundle {
            overview: "Clean run.".into(),
            recommendations: vec![AttackRecommendation {
                title: "Keep testing".into(),
                description: "Schedule continuous tests.".into(),
                priority: "info".into(),
                action: None,
            }],
        };
        let out = ensure_failed_scan_action_recommendation(&summary, bundle);
        assert_eq!(out.recommendations.len(), 1);
        assert!(out.recommendations[0].action.is_none());
    }
}
