//! Soft ReAct loop for Yazg: Thought → Action → Observation until `finish`.

use std::collections::HashMap;

use aisec_judge::{JudgeEngine, JudgeRequest};
use aisec_planner::PlannerLlm;
use aisec_target_profile::{AttackResultsSummary, TargetProfile, VerifyHttpSuccess};
use serde::Deserialize;
use tracing::{info, warn};

use crate::analyze_endpoint::{AnalyzeEndpointAgent, AnalyzeEndpointAgentOutcome};
use crate::attack_plan::{AttackPlanAgent, AttackPlanAgentOutcome};
use crate::error::{AgentError, AgentResult};
use crate::generate_prompt::{
    GeneratePromptAgent, GeneratePromptAgentOutcome, TechniquePromptContext,
};
use crate::judge_coordinator::{JudgeCoordinatorAgent, JudgeCoordinatorAgentOutcome};
use crate::recommend::{RecommendAgent, RecommendAgentOutcome};
use crate::summary::{SummaryAgent, SummaryAgentOutcome, SummaryRequest};
use crate::types::{AgentEvent, AgentId};

const DEFAULT_MAX_STEPS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactActionKind {
    AnalyzeEndpoint,
    AttackPlan,
    GeneratePrompt,
    Recommend,
    Summary,
    Judge,
    Finish,
}

#[derive(Debug, Clone, Default)]
pub struct ReactArtifacts {
    pub events: Vec<AgentEvent>,
    pub analyze: Option<AnalyzeEndpointAgentOutcome>,
    pub plan: Option<AttackPlanAgentOutcome>,
    pub generate_prompt: Option<GeneratePromptAgentOutcome>,
    pub recommend: Option<RecommendAgentOutcome>,
    pub summary: Option<SummaryAgentOutcome>,
    pub judge: Option<JudgeCoordinatorAgentOutcome>,
    pub final_reply: String,
    pub last_action: Option<ReactActionKind>,
}

/// LLM handles for each ReAct actor (supervisor + specialist sub-agents).
pub struct ReactLlms<'a> {
    pub supervisor: &'a dyn PlannerLlm,
    pub analyze: &'a dyn PlannerLlm,
    pub plan: &'a dyn PlannerLlm,
    pub prompt: &'a dyn PlannerLlm,
    pub recommend: &'a dyn PlannerLlm,
    pub summary: &'a dyn PlannerLlm,
}

#[derive(Clone)]
pub struct ReactRequest<'a> {
    pub goal: String,
    pub profile: Option<&'a TargetProfile>,
    pub auth_headers: HashMap<String, String>,
    /// When set, AnalyzeEndpointAgent classifies this probe (skips HTTP).
    pub capability_probe: Option<&'a VerifyHttpSuccess>,
    /// When set, GeneratePromptAgent can produce a factory probe.
    pub technique: Option<&'a TechniquePromptContext>,
    /// When set, RecommendAgent can produce post-scan recommendations.
    pub attack_results: Option<&'a AttackResultsSummary>,
    /// When set, SummaryAgent can produce project/scan summary.
    pub summary_request: Option<&'a SummaryRequest>,
    /// When set with `judge_engine`, JudgeCoordinatorAgent can score a probe.
    pub judge_request: Option<&'a JudgeRequest>,
    /// Runtime/role pool for JudgeCoordinatorAgent workers.
    pub judge_engine: Option<&'a JudgeEngine>,
    pub max_steps: usize,
}

impl<'a> ReactRequest<'a> {
    pub fn new(goal: impl Into<String>) -> Self {
        Self {
            goal: goal.into(),
            profile: None,
            auth_headers: HashMap::new(),
            capability_probe: None,
            technique: None,
            attack_results: None,
            summary_request: None,
            judge_request: None,
            judge_engine: None,
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

    pub fn with_technique(mut self, technique: Option<&'a TechniquePromptContext>) -> Self {
        self.technique = technique;
        self
    }

    pub fn with_attack_results(mut self, results: Option<&'a AttackResultsSummary>) -> Self {
        self.attack_results = results;
        self
    }

    pub fn with_summary_request(mut self, request: Option<&'a SummaryRequest>) -> Self {
        self.summary_request = request;
        self
    }

    pub fn with_judge(
        mut self,
        request: Option<&'a JudgeRequest>,
        engine: Option<&'a JudgeEngine>,
    ) -> Self {
        self.judge_request = request;
        self.judge_engine = engine;
        self
    }

    pub fn with_max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps.max(1);
        self
    }
}

#[derive(Debug, Deserialize)]
struct ReactStepJson {
    thought: Option<String>,
    action: String,
    #[serde(default)]
    reply: Option<String>,
}

pub async fn run_react(
    request: ReactRequest<'_>,
    llms: &ReactLlms<'_>,
) -> AgentResult<ReactArtifacts> {
    let mut artifacts = ReactArtifacts::default();
    artifacts.events.push(AgentEvent::started(
        AgentId::Yazg,
        "ReAct loop started",
    ));

    let mut transcript = String::new();
    transcript.push_str(&format!("Goal:\n{}\n\n", request.goal.trim()));
    transcript.push_str(&format_context(
        request.profile,
        request.capability_probe.is_some(),
        request.technique,
        request.attack_results.is_some(),
        request.summary_request,
        request.judge_request.is_some() && request.judge_engine.is_some(),
    ));
    transcript.push_str(
        "\nBegin ReAct. Respond with one JSON step (thought + action).\n",
    );

    for step in 1..=request.max_steps {
        info!(step, "Yazg ReAct step");
        artifacts.events.push(AgentEvent::info(
            AgentId::Yazg,
            format!("ReAct step {step}/{max}", max = request.max_steps),
        ));

        let raw = llms
            .supervisor
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
                    execute_analyze(&request, llms.analyze, &mut artifacts).await?;
                push_observation(&mut transcript, &mut artifacts, &observation);
            }
            ReactActionKind::AttackPlan => {
                let observation =
                    execute_attack_plan(&request, llms.plan, &mut artifacts).await?;
                push_observation(&mut transcript, &mut artifacts, &observation);
            }
            ReactActionKind::GeneratePrompt => {
                let observation =
                    execute_generate_prompt(&request, llms.prompt, &mut artifacts).await?;
                push_observation(&mut transcript, &mut artifacts, &observation);
            }
            ReactActionKind::Recommend => {
                let observation =
                    execute_recommend(&request, llms.recommend, &mut artifacts).await?;
                push_observation(&mut transcript, &mut artifacts, &observation);
            }
            ReactActionKind::Summary => {
                let observation =
                    execute_summary(&request, llms.summary, &mut artifacts).await?;
                push_observation(&mut transcript, &mut artifacts, &observation);
            }
            ReactActionKind::Judge => {
                let observation = execute_judge(&request, &mut artifacts).await?;
                push_observation(&mut transcript, &mut artifacts, &observation);
            }
        }

        transcript.push_str(
            "\nContinue ReAct: choose the next action JSON \
             (analyze_endpoint, attack_plan, generate_prompt, recommend, summary, judge, or finish).\n",
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

fn push_observation(
    transcript: &mut String,
    artifacts: &mut ReactArtifacts,
    observation: &str,
) {
    transcript.push_str(&format!("Observation:\n{observation}\n"));
    artifacts.events.push(AgentEvent::info(
        AgentId::Yazg,
        format!("Observation: {}", truncate(observation, 240)),
    ));
}

fn format_context(
    profile: Option<&TargetProfile>,
    has_probe: bool,
    technique: Option<&TechniquePromptContext>,
    has_attack_results: bool,
    summary_request: Option<&SummaryRequest>,
    judge_ready: bool,
) -> String {
    let mut out = match profile {
        Some(p) => format!(
            "Context:\n- target: {}\n- provider: {}\n- verified: {}\n- capability_probe_ready: {}\n",
            p.full_url(),
            p.provider.as_str(),
            p.is_verified(),
            has_probe
        ),
        None => "Context:\n- target: (none selected)\n- verified: false\n- capability_probe_ready: false\n"
            .into(),
    };
    if has_probe {
        out.push_str(
            "- note: capability_probe_ready means Scan wizard Verification — call analyze_endpoint; \
             this is NOT Attack Factory / generate_prompt\n",
        );
    }
    if let Some(t) = technique {
        out.push_str(&format!(
            "- technique_id: {}\n- technique_name: {}\n- category: {}\n- factory_prompt_ready: true\n\
             - note: Attack Factory generate_prompt does not need a scan target\n",
            t.id, t.name, t.category_id
        ));
    } else {
        out.push_str("- factory_prompt_ready: false\n");
    }
    out.push_str(&format!(
        "- attack_results_ready: {}\n",
        has_attack_results
    ));
    if has_attack_results {
        out.push_str(
            "- note: recommend uses completed scan results; no live target probe needed\n",
        );
    }
    match summary_request {
        Some(SummaryRequest::Project { project_name, .. }) => {
            out.push_str(&format!(
                "- summary_ready: true\n- summary_kind: project\n- project_name: {project_name}\n\
                 - note: summary does not need a live scan target\n"
            ));
        }
        Some(SummaryRequest::Scan { .. }) => {
            out.push_str(
                "- summary_ready: true\n- summary_kind: scan\n\
                 - note: summary uses completed scan results; no live target probe needed\n",
            );
        }
        None => out.push_str("- summary_ready: false\n"),
    }
    out.push_str(&format!("- judge_ready: {judge_ready}\n"));
    if judge_ready {
        out.push_str(
            "- note: judge runs JudgeCoordinatorAgent → JudgeWorker/ClassifierWorker/AttackerWorker; \
             no live target probe needed when probe response context is present\n",
        );
    }
    out
}

fn parse_action_kind(raw: &str) -> AgentResult<ReactActionKind> {
    match raw.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "analyze_endpoint" | "analyze" | "verify" | "verification" => {
            Ok(ReactActionKind::AnalyzeEndpoint)
        }
        "plan" | "planner" | "attack_plan" => Ok(ReactActionKind::AttackPlan),
        "generate_prompt"
        | "generate"
        | "prompt"
        | "factory_prompt"
        | "attack_factory" => Ok(ReactActionKind::GeneratePrompt),
        "recommend" | "recommendation" | "recommendations" | "remediation" => {
            Ok(ReactActionKind::Recommend)
        }
        "summary" | "summarize" | "project_summary" | "scan_summary" => {
            Ok(ReactActionKind::Summary)
        }
        "judge" | "judging" | "judge_coordinator" | "consensus_judge" => {
            Ok(ReactActionKind::Judge)
        }
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

async fn execute_generate_prompt(
    request: &ReactRequest<'_>,
    prompt_llm: &dyn PlannerLlm,
    artifacts: &mut ReactArtifacts,
) -> AgentResult<String> {
    let Some(technique) = request.technique else {
        return Ok("GeneratePromptAgent FAILED — no technique selected".into());
    };

    artifacts.events.push(AgentEvent::info(
        AgentId::Yazg,
        "Acting: GeneratePromptAgent",
    ));

    match GeneratePromptAgent::run(technique, prompt_llm).await {
        Ok(out) => {
            artifacts.events.extend(out.events.clone());
            let msg = format!(
                "GeneratePromptAgent OK — technique={} chars={} preview={}",
                out.technique_id,
                out.content.chars().count(),
                truncate(&out.content, 120)
            );
            artifacts.generate_prompt = Some(out);
            Ok(msg)
        }
        Err(err) => {
            artifacts.events.push(AgentEvent::failed(
                AgentId::GeneratePrompt,
                err.to_string(),
            ));
            Ok(format!("GeneratePromptAgent FAILED — {err}"))
        }
    }
}

async fn execute_recommend(
    request: &ReactRequest<'_>,
    recommend_llm: &dyn PlannerLlm,
    artifacts: &mut ReactArtifacts,
) -> AgentResult<String> {
    let Some(results) = request.attack_results else {
        return Ok("RecommendAgent FAILED — no attack results provided".into());
    };

    artifacts.events.push(AgentEvent::info(
        AgentId::Yazg,
        "Acting: RecommendAgent",
    ));

    match RecommendAgent::run(results, recommend_llm).await {
        Ok(out) => {
            artifacts.events.extend(out.events.clone());
            let msg = format!(
                "RecommendAgent OK — items={} overview={}",
                out.bundle.recommendations.len(),
                truncate(&out.bundle.overview, 160)
            );
            artifacts.recommend = Some(out);
            Ok(msg)
        }
        Err(err) => {
            artifacts.events.push(AgentEvent::failed(
                AgentId::Recommend,
                err.to_string(),
            ));
            Ok(format!("RecommendAgent FAILED — {err}"))
        }
    }
}

async fn execute_summary(
    request: &ReactRequest<'_>,
    summary_llm: &dyn PlannerLlm,
    artifacts: &mut ReactArtifacts,
) -> AgentResult<String> {
    let Some(summary_request) = request.summary_request else {
        return Ok("SummaryAgent FAILED — no summary request provided".into());
    };

    artifacts.events.push(AgentEvent::info(
        AgentId::Yazg,
        "Acting: SummaryAgent",
    ));

    match SummaryAgent::run(summary_request, summary_llm).await {
        Ok(out) => {
            artifacts.events.extend(out.events.clone());
            let msg = format!(
                "SummaryAgent OK — kind={} overview={} highlights={}",
                out.kind,
                truncate(&out.bundle.overview, 120),
                out.bundle.highlights.len()
            );
            artifacts.summary = Some(out);
            Ok(msg)
        }
        Err(err) => {
            artifacts
                .events
                .push(AgentEvent::failed(AgentId::Summary, err.to_string()));
            Ok(format!("SummaryAgent FAILED — {err}"))
        }
    }
}

async fn execute_judge(
    request: &ReactRequest<'_>,
    artifacts: &mut ReactArtifacts,
) -> AgentResult<String> {
    let (Some(judge_request), Some(engine)) = (request.judge_request, request.judge_engine) else {
        return Ok(
            "JudgeCoordinatorAgent FAILED — judge request/engine not provided".into(),
        );
    };

    artifacts.events.push(AgentEvent::info(
        AgentId::Yazg,
        "Acting: JudgeCoordinatorAgent",
    ));

    match JudgeCoordinatorAgent::run(judge_request, engine).await {
        Ok(out) => {
            artifacts.events.extend(out.events.clone());
            let msg = format!(
                "JudgeCoordinatorAgent OK — verdict={} confidence={:.2} votes={}",
                out.verdict.verdict,
                out.verdict.confidence,
                out.worker_results.len()
            );
            artifacts.judge = Some(out);
            Ok(msg)
        }
        Err(err) => {
            artifacts.events.push(AgentEvent::failed(
                AgentId::JudgeCoordinator,
                err.to_string(),
            ));
            Ok(format!("JudgeCoordinatorAgent FAILED — {err}"))
        }
    }
}

fn summarize_partial(artifacts: &ReactArtifacts) -> String {
    if let Some(judge) = &artifacts.judge {
        return format!(
            "Reached step limit after judging. Verdict: {} · confidence={:.2}",
            judge.verdict.verdict, judge.verdict.confidence
        );
    }
    if let Some(summary) = &artifacts.summary {
        return format!(
            "Reached step limit after summary. Kind: {} · {}",
            summary.kind,
            truncate(&summary.bundle.overview, 120)
        );
    }
    if let Some(rec) = &artifacts.recommend {
        return format!(
            "Reached step limit after recommendations. Items: {}",
            rec.bundle.recommendations.len()
        );
    }
    if let Some(gen) = &artifacts.generate_prompt {
        return format!(
            "Reached step limit after prompt generation. Technique: {} · {} chars",
            gen.technique_id,
            gen.content.chars().count()
        );
    }
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
            parse_action_kind("recommend").unwrap(),
            ReactActionKind::Recommend
        );
        assert_eq!(
            parse_action_kind("project_summary").unwrap(),
            ReactActionKind::Summary
        );
        assert_eq!(
            parse_action_kind("judge").unwrap(),
            ReactActionKind::Judge
        );
        assert_eq!(
            parse_action_kind("generate_prompt").unwrap(),
            ReactActionKind::GeneratePrompt
        );
    }
}
