//! LLM-backed remediation recommendations from attack scan findings.

use std::collections::HashMap;

use aisec_inference::PromptRegistry;
use aisec_planner::{PlannerError, PlannerLlm, PlannerResult};
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
}

#[derive(Debug, Deserialize)]
struct LlmRecommendationsResponse {
    recommendations: Vec<LlmRecommendationItem>,
}

#[derive(Debug, Deserialize)]
struct LlmRecommendationItem {
    title: String,
    description: String,
    priority: String,
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
        total_findings: findings.len(),
        severity_counts,
        attack_categories: attack_categories.to_vec(),
        findings: findings.to_vec(),
    }
}

pub async fn generate_attack_recommendations_with_llm(
    summary: &AttackResultsSummary,
    llm: &impl PlannerLlm,
) -> PlannerResult<Vec<AttackRecommendation>> {
    let summary_json = serde_json::to_string(summary)
        .map_err(|e| PlannerError::Llm(format!("failed to serialize findings summary: {e}")))?;
    let prompt = PromptRegistry::attack_results_recommend_user(&summary_json);
    let raw = llm.complete(&prompt).await?;
    parse_attack_recommendations(&raw)
}

fn parse_attack_recommendations(raw: &str) -> PlannerResult<Vec<AttackRecommendation>> {
    let json_str = extract_json_object(raw)?;
    let parsed: LlmRecommendationsResponse = serde_json::from_str(&json_str)
        .map_err(|e| PlannerError::Llm(format!("invalid recommendations JSON: {e}")))?;

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
            priority: normalize_priority(&item.priority),
        });
    }

    Ok(out)
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
        let raw = r#"{"recommendations":[{"title":"Guardrails","description":"Add input filters.","priority":"critical"}]}"#;
        let recs = parse_attack_recommendations(raw).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].title, "Guardrails");
        assert_eq!(recs[0].priority, "critical");
    }
}
