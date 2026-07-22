//! LLM-backed posture summaries (project + scan).

use aisec_inference::PromptRegistry;
use aisec_planner::{PlannerError, PlannerLlm, PlannerResult};
use serde::{Deserialize, Serialize};

use crate::wizard_recommendations::AttackResultsSummary;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryBundle {
    pub overview: String,
    pub highlights: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LlmSummaryResponse {
    overview: String,
    #[serde(default)]
    highlights: Vec<String>,
}

/// Project-level summary from a JSON payload (host builds the input object).
pub async fn generate_project_summary_with_llm(
    input_json: &str,
    llm: &dyn PlannerLlm,
) -> PlannerResult<SummaryBundle> {
    let prompt = PromptRegistry::project_summary_user(input_json);
    let raw = llm.complete(&prompt).await?;
    parse_summary_bundle(&raw)
}

/// Scan-level summary from an attack-results summary.
pub async fn generate_scan_summary_with_llm(
    summary: &AttackResultsSummary,
    llm: &dyn PlannerLlm,
) -> PlannerResult<SummaryBundle> {
    let summary_json = serde_json::to_string(summary)
        .map_err(|e| PlannerError::Llm(format!("failed to serialize scan summary: {e}")))?;
    let prompt = PromptRegistry::scan_summary_user(&summary_json);
    let raw = llm.complete(&prompt).await?;
    parse_summary_bundle(&raw)
}

pub fn parse_summary_bundle(raw: &str) -> PlannerResult<SummaryBundle> {
    let json_str = extract_json_object(raw)?;
    let parsed: LlmSummaryResponse = serde_json::from_str(&json_str)
        .map_err(|e| PlannerError::Llm(format!("invalid summary JSON: {e}")))?;

    let overview = parsed.overview.trim();
    if overview.is_empty() {
        return Err(PlannerError::Llm("summary overview must not be empty".into()));
    }
    let highlights: Vec<String> = parsed
        .highlights
        .into_iter()
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty())
        .take(6)
        .collect();
    if highlights.is_empty() {
        return Err(PlannerError::Llm(
            "summary highlights must not be empty".into(),
        ));
    }
    Ok(SummaryBundle {
        overview: overview.into(),
        highlights,
    })
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
    fn parse_summary_ok() {
        let raw = r#"{"overview":"Project posture is mixed.","highlights":["Hot target A","Coverage gap on B"]}"#;
        let bundle = parse_summary_bundle(raw).unwrap();
        assert!(bundle.overview.contains("mixed"));
        assert_eq!(bundle.highlights.len(), 2);
    }
}
