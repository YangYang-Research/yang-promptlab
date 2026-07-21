//! Yazg ReAct loop — Reason → Act → Observe until finish.

use std::collections::HashMap;

use aisec_planner::PlannerLlm;
use aisec_target_profile::{TargetProfile, VerifyHttpSuccess};
use serde::Deserialize;
use tracing::{info, warn};

use crate::analyze_endpoint::{AnalyzeEndpointAgent, AnalyzeEndpointAgentOutcome};
use crate::error::{AgentError, AgentResult};
use crate::attack_plan::{AttackPlanAgent, AttackPlanAgentOutcome};
use crate::types::{AgentEvent, AgentId};

const DEFAULT_MAX_STEPS: u32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReactActionKind {
    AnalyzeEndpoint,
    AttackPlan,
    Finish,
}

#[derive(Debug, Deserialize)]
struct ReactStepJson {
    thought: Option<String>,
    action: String,
    #[serde(default)]
    reply: Option<String>,
}

/// Inputs for one Yazg ReAct run.
pub struct ReactRequest<'a> {
    pub goal: String,
    pub profile: Option<&'a TargetProfile>,
    pub auth_headers: HashMap<String, String>,
    /// When set (wizard AI step), AnalyzeEndpointAgent classifies this probe.
    pub capability_probe: Option<&'a VerifyHttpSuccess>,
    pub max_steps: u32,
}

impl<'a> ReactRequest<'a> {
    pub fn new(goal: impl Into<String>) -> Self {
        Self {
            goal: goal.into(),
            profile: None,
            auth_headers: HashMap::new(),
            capability_probe: None,
            max_steps: DEFAULT_MAX_STEPS,
        }
    }

    pub fn with_profile(mut self, profile: Option<&'a TargetProfile>) -> Self {
        self.profile = profile;
        self
    }

    pub fn with_auth(mut self, auth_headers: HashMap<String, String>) -> Self {
        self.auth_headers = auth_headers;
        self
    }

    pub fn with_capability_probe(mut self, probe: Option<&'a VerifyHttpSuccess>) -> Self {
        self.capability_probe = probe;
        self
    }

    pub fn with_max_steps(mut self, max_steps: u32) -> Self {
        self.max_steps = max_steps.max(1);
        self
    }
}

/// Accumulated domain outcomes from actions during the loop.
#[derive(Debug, Default)]
pub struct ReactArtifacts {
    pub analyze: Option<AnalyzeEndpointAgentOutcome>,
    pub plan: Option<AttackPlanAgentOutcome>,
    pub events: Vec<AgentEvent>,
    pub final_reply: String,
    pub last_action: Option<ReactActionKind>,
}

/// Run Yazg ReAct until `finish` or max steps.
pub async fn run_react(
    request: ReactRequest<'_>,
    supervisor_llm: &dyn PlannerLlm,
    analyze_llm: &dyn PlannerLlm,
    plan_llm: &dyn PlannerLlm,
) -> AgentResult<ReactArtifacts> {
    let mut artifacts = ReactArtifacts::default();
    artifacts.events.push(AgentEvent::started(
        AgentId::Yazg,
        "ReAct loop started",
    ));

    let mut transcript = String::new();
    transcript.push_str(&format!("Goal:\n{}\n\n", request.goal.trim()));
    transcript.push_str(&format_context(request.profile, request.capability_probe.is_some()));
    transcript.push_str(
        "\nBegin ReAct. Respond with one JSON step (thought + action).\n",
    );

    for step in 1..=request.max_steps {
        info!(step, "Yazg ReAct step");
        artifacts.events.push(AgentEvent::info(
            AgentId::Yazg,
            format!("ReAct step {step}/{max}", max = request.max_steps),
        ));

        let raw = supervisor_llm
            .complete(&transcript)
            .await
            .map_err(|err| AgentError::Supervisor(format!("ReAct reasoning failed: {err}")))?;

        let parsed = parse_react_step(&raw).map_err(|err| {
            warn!(error = %err, raw = %truncate(&raw, 400), "Yazg ReAct parse failed");
            AgentError::Supervisor(err)
        })?;

        let thought = parsed
            .thought
            .unwrap_or_else(|| "(no thought)".into())
            .trim()
            .to_string();
        artifacts.events.push(AgentEvent::info(
            AgentId::Yazg,
            format!("Thought: {thought}"),
        ));

        let action = parse_action_kind(&parsed.action)?;
        artifacts.last_action = Some(action);

        transcript.push_str(&format!(
            "\n--- Step {step} ---\nThought: {thought}\nAction: {}\n",
            parsed.action
        ));

        match action {
            ReactActionKind::Finish => {
                let reply = parsed
                    .reply
                    .filter(|r| !r.trim().is_empty())
                    .unwrap_or_else(|| thought.clone());
                artifacts.final_reply = reply;
                artifacts.events.push(AgentEvent::completed(
                    AgentId::Yazg,
                    "ReAct finished",
                ));
                return Ok(artifacts);
            }
            ReactActionKind::AnalyzeEndpoint => {
                let observation =
                    execute_analyze(&request, analyze_llm, &mut artifacts).await?;
                transcript.push_str(&format!("Observation:\n{observation}\n"));
                artifacts.events.push(AgentEvent::info(
                    AgentId::Yazg,
                    format!("Observation: {}", truncate(&observation, 240)),
                ));
            }
            ReactActionKind::AttackPlan => {
                let observation = execute_attack_plan(&request, plan_llm, &mut artifacts).await?;
                transcript.push_str(&format!("Observation:\n{observation}\n"));
                artifacts.events.push(AgentEvent::info(
                    AgentId::Yazg,
                    format!("Observation: {}", truncate(&observation, 240)),
                ));
            }
        }

        transcript.push_str(
            "\nContinue ReAct: choose the next action JSON (analyze_endpoint, attack_plan, or finish).\n",
        );
    }

    artifacts.events.push(AgentEvent::failed(
        AgentId::Yazg,
        "ReAct reached max steps without finish",
    ));
    if artifacts.final_reply.trim().is_empty() {
        artifacts.final_reply = summarize_partial(&artifacts);
    }
    Ok(artifacts)
}

fn format_context(profile: Option<&TargetProfile>, has_probe: bool) -> String {
    match profile {
        Some(p) => format!(
            "Context:\n- target: {}\n- provider: {}\n- verified: {}\n- capability_probe_ready: {}\n",
            p.full_url(),
            p.provider.as_str(),
            p.is_verified(),
            has_probe
        ),
        None => "Context:\n- target: (none selected)\n- verified: false\n- capability_probe_ready: false\n"
            .into(),
    }
}

fn parse_action_kind(raw: &str) -> AgentResult<ReactActionKind> {
    match raw.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "analyze_endpoint" | "analyze" | "verify" | "verification" => {
            Ok(ReactActionKind::AnalyzeEndpoint)
        }
        "plan" | "planner" | "attack_plan" => Ok(ReactActionKind::AttackPlan),
        "finish" | "done" | "respond" | "final" => Ok(ReactActionKind::Finish),
        other => Err(AgentError::Supervisor(format!(
            "unknown ReAct action '{other}'"
        ))),
    }
}

fn parse_react_step(raw: &str) -> Result<ReactStepJson, String> {
    let trimmed = raw.trim();
    let json_slice = extract_json_object(trimmed).ok_or_else(|| {
        "ReAct response did not contain a JSON object".to_string()
    })?;
    serde_json::from_str(json_slice).map_err(|err| format!("invalid ReAct JSON: {err}"))
}

fn extract_json_object(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let mut depth = 0i32;
    for (idx, ch) in raw[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&raw[start..start + idx + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

async fn execute_analyze(
    request: &ReactRequest<'_>,
    analyze_llm: &dyn PlannerLlm,
    artifacts: &mut ReactArtifacts,
) -> AgentResult<String> {
    let Some(profile) = request.profile else {
        return Ok("AnalyzeEndpointAgent FAILED — no target selected".into());
    };

    artifacts.events.push(AgentEvent::info(
        AgentId::Yazg,
        "Acting: AnalyzeEndpointAgent",
    ));

    let outcome = if let Some(http) = request.capability_probe {
        AnalyzeEndpointAgent::classify_probe(profile, http, analyze_llm).await
    } else {
        AnalyzeEndpointAgent::run(profile, request.auth_headers.clone(), analyze_llm).await
    };

    match outcome {
        Ok(out) => {
            artifacts.events.extend(out.events.clone());
            let msg = format!(
                "AnalyzeEndpointAgent OK — verified=true status={} latency_ms={} provider={} model={}",
                out.verification.status_code,
                out.verification.response_time_ms,
                out.verification.provider,
                out.verification.model.as_deref().unwrap_or("unknown")
            );
            artifacts.analyze = Some(out);
            Ok(msg)
        }
        Err(err) => {
            artifacts.events.push(AgentEvent::failed(
                AgentId::AnalyzeEndpoint,
                err.to_string(),
            ));
            Ok(format!("AnalyzeEndpointAgent FAILED — {err}"))
        }
    }
}

async fn execute_attack_plan(
    request: &ReactRequest<'_>,
    plan_llm: &dyn PlannerLlm,
    artifacts: &mut ReactArtifacts,
) -> AgentResult<String> {
    let Some(profile) = request.profile else {
        return Ok("AttackPlanAgent FAILED — no target selected".into());
    };

    artifacts
        .events
        .push(AgentEvent::info(AgentId::Yazg, "Acting: AttackPlanAgent"));

    match AttackPlanAgent::run(profile, plan_llm).await {
        Ok(out) => {
            artifacts.events.extend(out.events.clone());
            let msg = format!(
                "AttackPlanAgent OK — categories={} modes={} source={} summary={}",
                out.plan.categories.len(),
                out.plan.profile_modes.len(),
                out.plan.planner_source,
                truncate(&out.plan.summary, 160)
            );
            artifacts.plan = Some(out);
            Ok(msg)
        }
        Err(err) => {
            artifacts
                .events
                .push(AgentEvent::failed(AgentId::AttackPlan, err.to_string()));
            Ok(format!("AttackPlanAgent FAILED — {err}"))
        }
    }
}

fn summarize_partial(artifacts: &ReactArtifacts) -> String {
    if let Some(plan) = &artifacts.plan {
        return format!(
            "Reached step limit after planning. Categories: {} · source: {}",
            plan.plan.categories.len(),
            plan.plan.planner_source
        );
    }
    if let Some(analyze) = &artifacts.analyze {
        return format!(
            "Reached step limit after endpoint analysis. Verified AI endpoint (HTTP {}).",
            analyze.verification.status_code
        );
    }
    "I could not finish the ReAct loop in time. Try again or narrow the request.".into()
}

fn truncate(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    let shortened: String = t.chars().take(max.saturating_sub(1)).collect();
    format!("{shortened}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_react_json() {
        let raw = r#"Here is my step:
{"thought":"Need to classify the API","action":"analyze_endpoint"}
"#;
        let step = parse_react_step(raw).expect("parse");
        assert_eq!(step.action, "analyze_endpoint");
        assert!(step.thought.unwrap().contains("classify"));
    }

    #[test]
    fn maps_action_aliases() {
        assert_eq!(
            parse_action_kind("verify").unwrap(),
            ReactActionKind::AnalyzeEndpoint
        );
        assert_eq!(parse_action_kind("finish").unwrap(), ReactActionKind::Finish);
        assert_eq!(
            parse_action_kind("plan").unwrap(),
            ReactActionKind::AttackPlan
        );
        assert_eq!(
            parse_action_kind("attack_plan").unwrap(),
            ReactActionKind::AttackPlan
        );
    }
}
