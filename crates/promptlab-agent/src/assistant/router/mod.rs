//! Pre-LLM capability router — first LLM call picks capability (no tools).
//!
//! Flow:
//! 1. LLM classifies user message → [`AssistantCapability`] (JSON, no tools)
//! 2. [`CapabilityToolLoader`] loads that capability's tool set
//! 3. Second LLM turn (Yazg agent) may call tools from that set only

use promptlab_planner::PlannerLlm;
use serde::Deserialize;
use tracing::{info, warn};

use super::capability_registry::AssistantCapability;

/// Router input — classifier uses **only** the latest user message (no history).
#[derive(Debug, Clone)]
pub struct RouteInput<'a> {
    pub latest_user_message: &'a str,
}

/// Result of intent routing (before tool-aware AI Runtime turn).
#[derive(Debug, Clone, PartialEq)]
pub struct IntentResolution {
    pub capability: AssistantCapability,
    pub confidence: f32,
    pub reason: String,
    /// Raw classifier model text (for Agent Trace / debug).
    pub raw_classifier_output: Option<String>,
    /// Full OpenAI-style request body sent to the classifier LLM.
    pub classifier_request: Option<serde_json::Value>,
}

/// LLM-based capability router. Runs **before** the tool-calling agent turn.
#[derive(Debug, Default, Clone)]
pub struct IntentRouter;

impl IntentRouter {
    pub fn new() -> Self {
        Self
    }

    /// Classify capability with a text-only LLM call (no tools bound).
    ///
    /// Only the latest user message is sent — conversation history is intentionally
    /// omitted so routing stays focused on the current turn.
    pub async fn resolve_with_llm(
        &self,
        llm: &dyn PlannerLlm,
        input: &RouteInput<'_>,
    ) -> IntentResolution {
        let latest = input.latest_user_message.trim();
        if latest.is_empty() {
            return IntentResolution {
                capability: AssistantCapability::Conversation,
                confidence: 1.0,
                reason: "empty_message".into(),
                raw_classifier_output: None,
                classifier_request: None,
            };
        }

        let user_prompt = format!("{latest}\n");
        let classifier_request = serde_json::json!({
            "messages": [
                { "role": "system", "content": CAPABILITY_CLASSIFIER_SYSTEM },
                { "role": "user", "content": user_prompt },
            ],
            "model_params": {
                "max_tokens": 1024,
                "temperature": 0.2,
                "tool_choice": null
            },
            "tools": []
        });
        let started = std::time::Instant::now();
        let raw = match llm
            .complete_with_system(Some(CAPABILITY_CLASSIFIER_SYSTEM), &user_prompt)
            .await
        {
            Ok(text) => text,
            Err(err) => {
                warn!(error = %err, "capability classifier LLM failed; defaulting to conversation");
                return IntentResolution {
                    capability: AssistantCapability::Conversation,
                    confidence: 0.4,
                    reason: format!("classifier_llm_error:{err}"),
                    raw_classifier_output: None,
                    classifier_request: Some(classifier_request),
                };
            }
        };
        let latency_ms = started.elapsed().as_millis();
        info!(
            latency_ms,
            raw = %truncate(&raw, 240),
            "yazg capability classifier response"
        );

        match parse_capability_response(&raw) {
            Some(mut resolution) => {
                if let Some(corrected) = correct_misrouted_capability(latest, resolution.capability)
                {
                    info!(
                        from = %resolution.capability,
                        to = %corrected,
                        "yazg capability classifier corrected by heuristic"
                    );
                    resolution.capability = corrected;
                    resolution.reason = format!("corrected:{};{}", corrected.as_str(), resolution.reason);
                    resolution.confidence = resolution.confidence.max(0.85);
                }
                resolution.raw_classifier_output = Some(raw);
                resolution.classifier_request = Some(classifier_request);
                resolution
            }
            None => {
                warn!(raw = %truncate(&raw, 240), "capability classifier parse failed");
                IntentResolution {
                    capability: AssistantCapability::Conversation,
                    confidence: 0.45,
                    reason: "classifier_parse_failed".into(),
                    raw_classifier_output: Some(raw),
                    classifier_request: Some(classifier_request),
                }
            }
        }
    }
}

const CAPABILITY_CLASSIFIER_SYSTEM: &str = r##"You are Yazg's capability router for PromptLab.
Classify the latest user message into exactly ONE capability.
Do NOT call tools. Do NOT answer the user. Output JSON only.

Capabilities:
- conversation — greetings, small talk, identity, thanks, how-are-you; no workspace data needed
- knowledge — conceptual AI/security Q&A only (prompt injection, OWASP, architecture). No PromptLab DB rows.
- workspace — inventory of which projects exist / workspace overview (no specific project name)
- projects — list / create / detail a PromptLab workspace project (often a named project)
- targets — list/detail targets or analyze a bound endpoint
- scan — list/detail/start/stop scans
- findings — findings / vulnerabilities
- reports — generated reports
- attack — attack plan, attack factory generate_prompt, recommend, summary, judge
- models — install/list/remove models
- runtime — AI runtime start/stop/status
- settings — app settings / preferences

Disambiguation (critical):
- If the user asks for info/detail/list about a "project" (or "dự án") in PromptLab — including a project name like "AI" — choose projects, NOT knowledge.
- knowledge is ONLY for abstract concepts that do not need workspace rows. Do not treat a named PromptLab project as a concept.
- Prefer projects over workspace when a project is named or the user asks to create one.
- Prefer conversation when unsure and no live workspace/runtime data is clearly required.
- Prefer targets when the user asks about endpoints/targets.

Examples:
- "hi" → conversation
- "what is prompt injection?" → knowledge
- "list projects" → projects
- "cho tôi thông tin project AI" → projects
- "project detail Foo" → projects
- "start a scan" → scan

Respond with a single JSON object only (no markdown fences), shape:
  {"capability":"<id>","confidence":0.0-1.0,"reason":"short"}"##;

#[derive(Debug, Deserialize)]
struct ClassifierJson {
    capability: String,
    #[serde(default)]
    confidence: Option<f32>,
    #[serde(default)]
    reason: Option<String>,
}

/// Parse classifier JSON (allows fenced ```json blocks).
pub fn parse_capability_response(raw: &str) -> Option<IntentResolution> {
    let json_str = extract_json_object(raw)?;
    let parsed: ClassifierJson = serde_json::from_str(&json_str).ok()?;
    let capability = parse_capability_id(&parsed.capability)?;
    let confidence = parsed.confidence.unwrap_or(0.7).clamp(0.0, 1.0);
    let reason = parsed
        .reason
        .unwrap_or_else(|| "llm_classifier".into())
        .chars()
        .take(200)
        .collect();
    Some(IntentResolution {
        capability,
        confidence,
        reason,
        raw_classifier_output: None,
        classifier_request: None,
    })
}

fn parse_capability_id(raw: &str) -> Option<AssistantCapability> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "conversation" | "chat" | "none" => Some(AssistantCapability::Conversation),
        "knowledge" | "qa" | "concept" => Some(AssistantCapability::Knowledge),
        "workspace" => Some(AssistantCapability::Workspace),
        "projects" | "project" => Some(AssistantCapability::Projects),
        "targets" | "target" | "endpoint" => Some(AssistantCapability::Targets),
        "scan" | "scans" => Some(AssistantCapability::Scan),
        "findings" | "finding" | "vuln" | "vulnerability" => Some(AssistantCapability::Findings),
        "reports" | "report" => Some(AssistantCapability::Reports),
        "attack" | "factory" | "judge" | "recommend" | "summary" => {
            Some(AssistantCapability::Attack)
        }
        "models" | "model" => Some(AssistantCapability::Models),
        "runtime" => Some(AssistantCapability::Runtime),
        "settings" | "setting" => Some(AssistantCapability::Settings),
        _ => None,
    }
}

fn extract_json_object(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let unfenced = if let Some(rest) = trimmed.strip_prefix("```") {
        let rest = rest
            .trim_start_matches(|c: char| c.is_ascii_alphanumeric())
            .trim_start_matches('\n');
        rest.strip_suffix("```").unwrap_or(rest).trim()
    } else {
        trimmed
    };
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(unfenced) {
        if v.is_object() {
            return Some(unfenced.to_string());
        }
    }
    let start = unfenced.find('{')?;
    let end = unfenced.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(unfenced[start..=end].to_string())
}

fn truncate(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    let cut: String = t.chars().take(max.saturating_sub(1)).collect();
    format!("{cut}…")
}

/// Fix common LLM mistakes: workspace project asks routed to knowledge/conversation.
fn correct_misrouted_capability(
    latest: &str,
    capability: AssistantCapability,
) -> Option<AssistantCapability> {
    if !matches!(
        capability,
        AssistantCapability::Knowledge | AssistantCapability::Conversation | AssistantCapability::Workspace
    ) {
        return None;
    }
    if looks_like_promptlab_project_request(latest) {
        return Some(AssistantCapability::Projects);
    }
    None
}

fn looks_like_promptlab_project_request(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    let has_project_word = lower.contains("project")
        || lower.contains("projects")
        || lower.contains("dự án")
        || lower.contains("du an");
    if !has_project_word {
        return false;
    }
    // Pure conceptual questions stay on knowledge.
    if is_conceptual_only(&lower) {
        return false;
    }
    // Explicit PromptLab project actions / lookups.
    const ACTION_MARKERS: &[&str] = &[
        "thông tin",
        "thong tin",
        "chi tiết",
        "chi tiet",
        "detail",
        "details",
        "info",
        "information",
        "list",
        "show",
        "get",
        "open",
        "create",
        "new",
        "tạo",
        "tao",
        "xem",
        "cho tôi",
        "cho toi",
        "về project",
        "ve project",
        "about project",
    ];
    if ACTION_MARKERS.iter().any(|m| lower.contains(m)) {
        return true;
    }
    // "project <Name>" / "<Name> project" style (name is not a concept word).
    named_project_mention(&lower)
}

fn is_conceptual_only(lower: &str) -> bool {
    const CONCEPT_MARKERS: &[&str] = &[
        "what is",
        "what's",
        "whats",
        "explain",
        "define",
        "definition",
        "how does",
        "why is",
        "là gì",
        "la gi",
        "giải thích",
        "giai thich",
        "nghĩa là",
        "nghia la",
        "concept",
        "theory",
        "methodology",
        "best practice",
        "owasp",
        "prompt injection",
    ];
    CONCEPT_MARKERS.iter().any(|m| lower.contains(m))
        && !lower.contains("thông tin project")
        && !lower.contains("thong tin project")
        && !lower.contains("project detail")
        && !lower.contains("list project")
}

fn named_project_mention(lower: &str) -> bool {
    // project <token> where token is not a concept filler
    const STOP: &[&str] = &[
        "management",
        "planning",
        "plan",
        "lifecycle",
        "methodology",
        "manager",
        "based",
        "in",
        "a",
        "an",
        "the",
        "my",
        "our",
        "this",
        "that",
        "of",
        "for",
        "on",
        "to",
        "và",
        "va",
        "của",
        "cua",
    ];
    for (idx, _) in lower.match_indices("project") {
        let after = lower[idx + "project".len()..].trim_start();
        let token = after
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .next()
            .unwrap_or("");
        if !token.is_empty() && !STOP.contains(&token) && token != "s" {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use promptlab_planner::{PlannerError, PlannerResult};
    use std::sync::Mutex;

    struct ScriptedLlm {
        response: Mutex<String>,
    }

    #[async_trait]
    impl PlannerLlm for ScriptedLlm {
        async fn complete(&self, _prompt: &str) -> PlannerResult<String> {
            Ok(self.response.lock().unwrap().clone())
        }
    }

    #[test]
    fn parses_plain_json() {
        let r = parse_capability_response(
            r#"{"capability":"projects","confidence":0.91,"reason":"list projects"}"#,
        )
        .expect("parse");
        assert_eq!(r.capability, AssistantCapability::Projects);
        assert!((r.confidence - 0.91).abs() < 0.001);
    }

    #[test]
    fn parses_fenced_json() {
        let r = parse_capability_response(
            "```json\n{\"capability\":\"conversation\",\"confidence\":0.99,\"reason\":\"greeting\"}\n```",
        )
        .expect("parse");
        assert_eq!(r.capability, AssistantCapability::Conversation);
    }

    #[test]
    fn parses_knowledge_and_scan() {
        assert_eq!(
            parse_capability_response(r#"{"capability":"knowledge","confidence":0.8}"#)
                .unwrap()
                .capability,
            AssistantCapability::Knowledge
        );
        assert_eq!(
            parse_capability_response(r#"{"capability":"scan","confidence":0.8}"#)
                .unwrap()
                .capability,
            AssistantCapability::Scan
        );
    }

    #[test]
    fn unknown_capability_is_none() {
        assert!(parse_capability_response(r#"{"capability":"aliens"}"#).is_none());
    }

    #[test]
    fn corrects_project_info_misrouted_to_knowledge() {
        assert_eq!(
            correct_misrouted_capability(
                "cho tôi thông tin project AI",
                AssistantCapability::Knowledge
            ),
            Some(AssistantCapability::Projects)
        );
        assert_eq!(
            correct_misrouted_capability(
                "what is prompt injection?",
                AssistantCapability::Knowledge
            ),
            None
        );
        assert_eq!(
            correct_misrouted_capability("list projects", AssistantCapability::Conversation),
            Some(AssistantCapability::Projects)
        );
    }

    #[tokio::test]
    async fn llm_router_uses_model_json() {
        let llm = ScriptedLlm {
            response: Mutex::new(
                r#"{"capability":"projects","confidence":0.93,"reason":"asks for projects"}"#
                    .into(),
            ),
        };
        let r = IntentRouter::new()
            .resolve_with_llm(
                &llm,
                &RouteInput {
                    latest_user_message: "List projects",
                },
            )
            .await;
        assert_eq!(r.capability, AssistantCapability::Projects);
        assert!(r.raw_classifier_output.is_some());
    }

    #[tokio::test]
    async fn llm_router_falls_back_on_bad_json() {
        let llm = ScriptedLlm {
            response: Mutex::new("not json at all".into()),
        };
        let r = IntentRouter::new()
            .resolve_with_llm(
                &llm,
                &RouteInput {
                    latest_user_message: "hello",
                },
            )
            .await;
        assert_eq!(r.capability, AssistantCapability::Conversation);
        assert_eq!(r.reason, "classifier_parse_failed");
    }
}
