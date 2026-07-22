//! LLM-backed posture summaries (project + scan).

use aisec_inference::PromptRegistry;
use aisec_planner::{PlannerError, PlannerLlm, PlannerResult};
use serde::{Deserialize, Serialize};

use crate::wizard_recommendations::AttackResultsSummary;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryBundle {
    pub overview: String,
    pub highlights: Vec<String>,
    /// Optional UI actions (e.g. retry_scan when a project has failed scans).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<SummaryAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryAction {
    pub title: String,
    pub description: String,
    /// `retry_scan` | `start_attack`
    pub action: String,
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
        actions: Vec::new(),
    })
}

/// Whether project-level summary should surface Retry Scan / Start Attack.
pub fn project_has_retryable_scan_status<'a, I>(statuses: I) -> bool
where
    I: IntoIterator<Item = &'a str>,
{
    statuses
        .into_iter()
        .any(crate::wizard_recommendations::is_retryable_scan_status)
}

/// Attach a typed Retry Scan action when failed scans exist.
/// Does **not** rewrite highlight text — that must come from the LLM (or fallback)
/// as one of the normal highlight items.
pub fn ensure_failed_project_summary_action(
    has_retryable_scan: bool,
    mut bundle: SummaryBundle,
) -> SummaryBundle {
    if !has_retryable_scan {
        return bundle;
    }

    if !bundle
        .actions
        .iter()
        .any(|a| matches!(a.action.as_str(), "retry_scan" | "start_attack"))
    {
        bundle.actions = vec![SummaryAction {
            title: "Retry Scan".into(),
            description: "Open the scan wizard to review plan/auth and Retry Scan on the \
                 newest failed assessment."
                .into(),
            action: "retry_scan".into(),
        }];
    }

    order_summary_action_highlight_first(bundle)
}

fn highlight_looks_like_retry(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    t.contains("retry scan")
        || t.contains("re-run")
        || t.contains("rerun")
        || t.contains("start attack")
        || (t.contains("failed scan") && (t.contains("re-run") || t.contains("retry")))
}

fn order_summary_action_highlight_first(mut bundle: SummaryBundle) -> SummaryBundle {
    if let Some(idx) = bundle
        .highlights
        .iter()
        .position(|h| highlight_looks_like_retry(h))
    {
        if idx > 0 {
            let item = bundle.highlights.remove(idx);
            bundle.highlights.insert(0, item);
        }
    }
    bundle
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
        assert!(bundle.actions.is_empty());
    }

    #[test]
    fn ensure_attaches_action_without_rewriting_highlights() {
        let bundle = SummaryBundle {
            overview: "Scan failed.".into(),
            highlights: vec![
                "Retry Scan on https://api.example.com/v1 after auth fix for scan abc.".into(),
                "Verify auth".into(),
                "Cover high-value targets".into(),
                "Establish cadence".into(),
            ],
            actions: Vec::new(),
        };
        let out = ensure_failed_project_summary_action(true, bundle);
        assert_eq!(out.actions.len(), 1);
        assert_eq!(out.actions[0].action, "retry_scan");
        assert!(out.highlights[0].contains("https://api.example.com/v1"));
        assert_eq!(out.highlights.len(), 4);
    }

    #[test]
    fn ensure_skips_when_no_failed() {
        let bundle = SummaryBundle {
            overview: "Clean.".into(),
            highlights: vec!["Keep testing".into()],
            actions: Vec::new(),
        };
        let out = ensure_failed_project_summary_action(false, bundle);
        assert!(out.actions.is_empty());
        assert_eq!(out.highlights.len(), 1);
    }
}
