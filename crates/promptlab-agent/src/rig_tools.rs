//! Rig tools for Yazg supervisor (workspace CRUD + specialist sub-agents).

use std::collections::HashMap;
use std::sync::Arc;

use promptlab_judge::{JudgeEngine, JudgeRequest};
use promptlab_planner::PlannerLlm;
use promptlab_target_profile::{AttackResultsSummary, TargetProfile, VerifyHttpSuccess};
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use tokio::sync::Mutex;

use crate::analyze_endpoint::AnalyzeEndpointAgent;
use crate::attack_plan::AttackPlanAgent;
use crate::create_project::{CreateProjectTools, CreatedProject};
use crate::generate_prompt::{GeneratePromptAgent, TechniquePromptContext};
use crate::judge_coordinator::JudgeCoordinatorAgent;
use crate::list_workspace::{WorkspaceInventory, WorkspaceTools};
use crate::artifacts::YazgArtifacts;
use crate::recommend::RecommendAgent;
use crate::summary::{SummaryAgent, SummaryRequest};
use crate::types::{AgentEvent, AgentId};

/// Shared mutable run state filled by Rig tools and consumed after `agent.prompt`.
#[derive(Default)]
pub struct YazgRigRunState {
    pub artifacts: YazgArtifacts,
    pub last_tool: Option<String>,
}

pub type SharedYazgRigState = Arc<Mutex<YazgRigRunState>>;

/// Owned specialist inputs for one Rig turn (wizard / chat with bound context).
#[derive(Default, Clone)]
pub struct YazgSpecialistContext {
    pub profile: Option<TargetProfile>,
    pub auth_headers: HashMap<String, String>,
    pub capability_probe: Option<VerifyHttpSuccess>,
    pub technique: Option<TechniquePromptContext>,
    pub attack_results: Option<AttackResultsSummary>,
    pub summary_request: Option<SummaryRequest>,
    pub judge_request: Option<JudgeRequest>,
    pub judge_engine: Option<Arc<JudgeEngine>>,
}

impl YazgSpecialistContext {
    pub fn with_profile(mut self, profile: TargetProfile) -> Self {
        self.profile = Some(profile);
        self
    }

    pub fn with_auth(mut self, auth_headers: HashMap<String, String>) -> Self {
        self.auth_headers = auth_headers;
        self
    }

    pub fn with_capability_probe(mut self, probe: VerifyHttpSuccess) -> Self {
        self.capability_probe = Some(probe);
        self
    }

    pub fn with_technique(mut self, technique: TechniquePromptContext) -> Self {
        self.technique = Some(technique);
        self
    }

    pub fn with_attack_results(mut self, results: AttackResultsSummary) -> Self {
        self.attack_results = Some(results);
        self
    }

    pub fn with_summary_request(mut self, request: SummaryRequest) -> Self {
        self.summary_request = Some(request);
        self
    }

    pub fn with_judge(mut self, request: JudgeRequest, engine: Arc<JudgeEngine>) -> Self {
        self.judge_request = Some(request);
        self.judge_engine = Some(engine);
        self
    }

    pub fn format_context_block(&self) -> String {
        let has_probe = self.capability_probe.is_some();
        let mut out = match self.profile.as_ref() {
            Some(p) => format!(
                "Runtime context:\n- bound_target: {}\n- provider: {}\n- verified: {}\n- capability_probe_ready: {}\n",
                p.full_url(),
                p.provider.as_str(),
                p.is_verified(),
                has_probe
            ),
            None => "Runtime context:\n- bound_target: (none)\n- verified: false\n- capability_probe_ready: false\n"
                .into(),
        };
        if has_probe {
            out.push_str(
                "- note: capability_probe_ready means Scan wizard Verification — call analyze_endpoint; \
                 this is NOT Attack Factory / generate_prompt\n",
            );
        }
        if let Some(t) = self.technique.as_ref() {
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
            self.attack_results.is_some()
        ));
        if self.attack_results.is_some() {
            out.push_str(
                "- note: recommend uses completed scan results; no live target probe needed\n",
            );
        }
        match self.summary_request.as_ref() {
            Some(SummaryRequest::Project { project_name, .. }) => {
                out.push_str(&format!(
                    "- summary_ready: true\n- summary_kind: project\n- project_name: {project_name}\n"
                ));
            }
            Some(SummaryRequest::Scan { .. }) => {
                out.push_str("- summary_ready: true\n- summary_kind: scan\n");
            }
            None => out.push_str("- summary_ready: false\n"),
        }
        let judge_ready = self.judge_request.is_some() && self.judge_engine.is_some();
        out.push_str(&format!("- judge_ready: {judge_ready}\n"));
        if judge_ready {
            out.push_str(
                "- note: judge runs JudgeCoordinatorAgent → JudgeWorker/ClassifierWorker/AttackerWorker\n",
            );
        }
        out
    }
}

/// Arc LLM handles for Rig supervisor + specialists.
#[derive(Clone)]
pub struct YazgRigLlms {
    pub supervisor: Arc<dyn PlannerLlm>,
    pub analyze: Arc<dyn PlannerLlm>,
    pub plan: Arc<dyn PlannerLlm>,
    pub prompt: Arc<dyn PlannerLlm>,
    pub recommend: Arc<dyn PlannerLlm>,
    pub summary: Arc<dyn PlannerLlm>,
}

impl YazgRigLlms {
    pub fn supervisor_only(supervisor: Arc<dyn PlannerLlm>) -> Self {
        Self {
            analyze: supervisor.clone(),
            plan: supervisor.clone(),
            prompt: supervisor.clone(),
            recommend: supervisor.clone(),
            summary: supervisor.clone(),
            supervisor,
        }
    }
}

#[derive(Debug, Error)]
#[error("{0}")]
pub struct YazgToolError(pub String);

#[derive(Deserialize, Serialize, Default)]
pub struct EmptyArgs {}

#[derive(Deserialize, Serialize, Default)]
pub struct ThoughtArgs {
    #[serde(default)]
    pub thought: Option<String>,
}

#[derive(Deserialize, Serialize, Default)]
pub struct CreateProjectArgs {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub thought: Option<String>,
}

fn thought_params() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "thought": { "type": "string", "description": "Brief reasoning" }
        },
        "additionalProperties": false
    })
}

fn truncate(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    let shortened: String = t.chars().take(max.saturating_sub(1)).collect();
    format!("{shortened}…")
}

async fn mark_tool(state: &SharedYazgRigState, name: &str, action: crate::artifacts::YazgActionKind) {
    let mut guard = state.lock().await;
    guard.last_tool = Some(name.into());
    guard.artifacts.last_action = Some(action);
}

pub struct ListWorkspaceRigTool {
    pub tools: Arc<dyn WorkspaceTools>,
    pub state: SharedYazgRigState,
}

impl Tool for ListWorkspaceRigTool {
    const NAME: &'static str = "list_workspace";
    type Error = YazgToolError;
    type Args = EmptyArgs;
    type Output = String;

    fn description(&self) -> String {
        "Read projects, targets, scans, and findings from the local PromptLab database. \
         ONLY when the user explicitly asks for inventory or finding/vulnerability counts. \
         Do NOT use for greetings, math, or general chat."
            .into()
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let inventory = self
            .tools
            .list_workspace()
            .await
            .map_err(YazgToolError)?;
        let observation = inventory.to_observation();
        mark_tool(
            &self.state,
            Self::NAME,
            crate::artifacts::YazgActionKind::ListWorkspace,
        )
        .await;
        let mut guard = self.state.lock().await;
        guard.artifacts.events.push(AgentEvent::info(
            AgentId::Yazg,
            "Acting: ListWorkspaceTool (Rig)",
        ));
        guard.artifacts.events.push(AgentEvent::completed(
            AgentId::ListWorkspace,
            format!(
                "Listed workspace: {} projects, {} targets, {} scans, {} findings",
                inventory.totals.projects,
                inventory.totals.targets,
                inventory.totals.scans,
                inventory.totals.findings
            ),
        ));
        guard.artifacts.workspace_inventory = Some(inventory);
        Ok(observation)
    }
}

pub struct CreateProjectRigTool {
    pub tools: Arc<dyn CreateProjectTools>,
    pub state: SharedYazgRigState,
}

impl Tool for CreateProjectRigTool {
    const NAME: &'static str = "create_project";
    type Error = YazgToolError;
    type Args = CreateProjectArgs;
    type Output = String;

    fn description(&self) -> String {
        "Create a workspace project in the local database. Requires a project name. \
         Optional description. Do NOT ask for a scan target."
            .into()
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "Project name to create" },
                "description": { "type": "string", "description": "Optional project description" }
            },
            "required": ["name"],
            "additionalProperties": false
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let name = args.name.trim();
        if name.is_empty() {
            return Err(YazgToolError(
                "missing project name — provide JSON field \"name\"".into(),
            ));
        }
        let description = args
            .description
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let project = self
            .tools
            .create_project(name, description)
            .await
            .map_err(YazgToolError)?;
        let msg = format!(
            "CreateProjectTool OK — id={} name={} description={}",
            project.id,
            project.name,
            project.description.as_deref().unwrap_or("(none)")
        );
        mark_tool(
            &self.state,
            Self::NAME,
            crate::artifacts::YazgActionKind::CreateProject,
        )
        .await;
        let mut guard = self.state.lock().await;
        guard.artifacts.events.push(AgentEvent::completed(
            AgentId::CreateProject,
            format!("Created project {}", project.name),
        ));
        guard.artifacts.created_project = Some(CreatedProject {
            id: project.id,
            name: project.name,
            description: project.description,
        });
        Ok(msg)
    }
}

pub struct AnalyzeEndpointRigTool {
    pub ctx: Arc<YazgSpecialistContext>,
    pub llm: Arc<dyn PlannerLlm>,
    pub state: SharedYazgRigState,
}

impl Tool for AnalyzeEndpointRigTool {
    const NAME: &'static str = "analyze_endpoint";
    type Error = YazgToolError;
    type Args = ThoughtArgs;
    type Output = String;

    fn description(&self) -> String {
        "Probe/classify whether a bound live scan target is a generative AI API \
         (AnalyzeEndpointAgent). Requires a bound target or capability_probe_ready=true \
         (Scan wizard Verification). Do NOT use for counting findings or general chat."
            .into()
    }

    fn parameters(&self) -> serde_json::Value {
        thought_params()
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let Some(profile) = self.ctx.profile.as_ref() else {
            return Ok("AnalyzeEndpointAgent FAILED — no target selected".into());
        };
        {
            let mut guard = self.state.lock().await;
            guard.artifacts.events.push(AgentEvent::info(
                AgentId::Yazg,
                "Acting: AnalyzeEndpointAgent (Rig)",
            ));
        }
        let outcome = if let Some(http) = self.ctx.capability_probe.as_ref() {
            AnalyzeEndpointAgent::classify_probe(profile, http, self.llm.as_ref()).await
        } else {
            AnalyzeEndpointAgent::run(profile, self.ctx.auth_headers.clone(), self.llm.as_ref())
                .await
        };
        mark_tool(
            &self.state,
            Self::NAME,
            crate::artifacts::YazgActionKind::AnalyzeEndpoint,
        )
        .await;
        match outcome {
            Ok(out) => {
                let msg = format!(
                    "AnalyzeEndpointAgent OK — verified=true status={} latency_ms={} provider={} model={}",
                    out.verification.status_code,
                    out.verification.response_time_ms,
                    out.verification.provider,
                    out.verification.model.as_deref().unwrap_or("unknown")
                );
                let mut guard = self.state.lock().await;
                guard.artifacts.events.extend(out.events.clone());
                guard.artifacts.analyze = Some(out);
                Ok(msg)
            }
            Err(err) => {
                let mut guard = self.state.lock().await;
                guard.artifacts.events.push(AgentEvent::failed(
                    AgentId::AnalyzeEndpoint,
                    err.to_string(),
                ));
                Ok(format!("AnalyzeEndpointAgent FAILED — {err}"))
            }
        }
    }
}

pub struct AttackPlanRigTool {
    pub ctx: Arc<YazgSpecialistContext>,
    pub llm: Arc<dyn PlannerLlm>,
    pub state: SharedYazgRigState,
}

impl Tool for AttackPlanRigTool {
    const NAME: &'static str = "attack_plan";
    type Error = YazgToolError;
    type Args = ThoughtArgs;
    type Output = String;

    fn description(&self) -> String {
        "Build an attack plan for a verified bound target (AttackPlanAgent). \
         Requires verified=true (or a bound verified target)."
            .into()
    }

    fn parameters(&self) -> serde_json::Value {
        thought_params()
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let Some(profile) = self.ctx.profile.as_ref() else {
            return Ok("AttackPlanAgent FAILED — no target selected".into());
        };
        {
            let mut guard = self.state.lock().await;
            guard
                .artifacts
                .events
                .push(AgentEvent::info(AgentId::Yazg, "Acting: AttackPlanAgent (Rig)"));
        }
        mark_tool(
            &self.state,
            Self::NAME,
            crate::artifacts::YazgActionKind::AttackPlan,
        )
        .await;
        match AttackPlanAgent::run(profile, self.llm.as_ref()).await {
            Ok(out) => {
                let msg = format!(
                    "AttackPlanAgent OK — categories={} modes={} source={} summary={}",
                    out.plan.categories.len(),
                    out.plan.profile_modes.len(),
                    out.plan.planner_source,
                    truncate(&out.plan.summary, 160)
                );
                let mut guard = self.state.lock().await;
                guard.artifacts.events.extend(out.events.clone());
                guard.artifacts.plan = Some(out);
                Ok(msg)
            }
            Err(err) => {
                let mut guard = self.state.lock().await;
                guard
                    .artifacts
                    .events
                    .push(AgentEvent::failed(AgentId::AttackPlan, err.to_string()));
                Ok(format!("AttackPlanAgent FAILED — {err}"))
            }
        }
    }
}

pub struct GeneratePromptRigTool {
    pub ctx: Arc<YazgSpecialistContext>,
    pub llm: Arc<dyn PlannerLlm>,
    pub state: SharedYazgRigState,
}

impl Tool for GeneratePromptRigTool {
    const NAME: &'static str = "generate_prompt";
    type Error = YazgToolError;
    type Args = ThoughtArgs;
    type Output = String;

    fn description(&self) -> String {
        "Attack Factory: invent a novel technique probe (GeneratePromptAgent). \
         Use only when factory_prompt_ready=true. Does not require a scan target."
            .into()
    }

    fn parameters(&self) -> serde_json::Value {
        thought_params()
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let Some(technique) = self.ctx.technique.as_ref() else {
            return Ok("GeneratePromptAgent FAILED — no technique selected".into());
        };
        {
            let mut guard = self.state.lock().await;
            guard.artifacts.events.push(AgentEvent::info(
                AgentId::Yazg,
                "Acting: GeneratePromptAgent (Rig)",
            ));
        }
        mark_tool(
            &self.state,
            Self::NAME,
            crate::artifacts::YazgActionKind::GeneratePrompt,
        )
        .await;
        match GeneratePromptAgent::run(technique, self.llm.as_ref()).await {
            Ok(out) => {
                let msg = format!(
                    "GeneratePromptAgent OK — technique={} chars={} preview={}",
                    out.technique_id,
                    out.content.chars().count(),
                    truncate(&out.content, 120)
                );
                let mut guard = self.state.lock().await;
                guard.artifacts.events.extend(out.events.clone());
                guard.artifacts.generate_prompt = Some(out);
                Ok(msg)
            }
            Err(err) => {
                let mut guard = self.state.lock().await;
                guard.artifacts.events.push(AgentEvent::failed(
                    AgentId::GeneratePrompt,
                    err.to_string(),
                ));
                Ok(format!("GeneratePromptAgent FAILED — {err}"))
            }
        }
    }
}

pub struct RecommendRigTool {
    pub ctx: Arc<YazgSpecialistContext>,
    pub llm: Arc<dyn PlannerLlm>,
    pub state: SharedYazgRigState,
}

impl Tool for RecommendRigTool {
    const NAME: &'static str = "recommend";
    type Error = YazgToolError;
    type Args = ThoughtArgs;
    type Output = String;

    fn description(&self) -> String {
        "Post-scan remediation recommendations from completed attack results \
         (RecommendAgent). Requires attack_results_ready=true."
            .into()
    }

    fn parameters(&self) -> serde_json::Value {
        thought_params()
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let Some(results) = self.ctx.attack_results.as_ref() else {
            return Ok("RecommendAgent FAILED — no attack results provided".into());
        };
        {
            let mut guard = self.state.lock().await;
            guard
                .artifacts
                .events
                .push(AgentEvent::info(AgentId::Yazg, "Acting: RecommendAgent (Rig)"));
        }
        mark_tool(
            &self.state,
            Self::NAME,
            crate::artifacts::YazgActionKind::Recommend,
        )
        .await;
        match RecommendAgent::run(results, self.llm.as_ref()).await {
            Ok(out) => {
                let msg = format!(
                    "RecommendAgent OK — items={} overview={}",
                    out.bundle.recommendations.len(),
                    truncate(&out.bundle.overview, 160)
                );
                let mut guard = self.state.lock().await;
                guard.artifacts.events.extend(out.events.clone());
                guard.artifacts.recommend = Some(out);
                Ok(msg)
            }
            Err(err) => {
                let mut guard = self.state.lock().await;
                guard
                    .artifacts
                    .events
                    .push(AgentEvent::failed(AgentId::Recommend, err.to_string()));
                Ok(format!("RecommendAgent FAILED — {err}"))
            }
        }
    }
}

pub struct SummaryRigTool {
    pub ctx: Arc<YazgSpecialistContext>,
    pub llm: Arc<dyn PlannerLlm>,
    pub state: SharedYazgRigState,
}

impl Tool for SummaryRigTool {
    const NAME: &'static str = "summary";
    type Error = YazgToolError;
    type Args = ThoughtArgs;
    type Output = String;

    fn description(&self) -> String {
        "Project or scan posture overview + highlights (SummaryAgent). \
         Requires summary_ready=true."
            .into()
    }

    fn parameters(&self) -> serde_json::Value {
        thought_params()
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let Some(summary_request) = self.ctx.summary_request.as_ref() else {
            return Ok("SummaryAgent FAILED — no summary request provided".into());
        };
        {
            let mut guard = self.state.lock().await;
            guard
                .artifacts
                .events
                .push(AgentEvent::info(AgentId::Yazg, "Acting: SummaryAgent (Rig)"));
        }
        mark_tool(&self.state, Self::NAME, crate::artifacts::YazgActionKind::Summary).await;
        match SummaryAgent::run(summary_request, self.llm.as_ref()).await {
            Ok(out) => {
                let msg = format!(
                    "SummaryAgent OK — kind={} overview={} highlights={}",
                    out.kind,
                    truncate(&out.bundle.overview, 120),
                    out.bundle.highlights.len()
                );
                let mut guard = self.state.lock().await;
                guard.artifacts.events.extend(out.events.clone());
                guard.artifacts.summary = Some(out);
                Ok(msg)
            }
            Err(err) => {
                let mut guard = self.state.lock().await;
                guard
                    .artifacts
                    .events
                    .push(AgentEvent::failed(AgentId::Summary, err.to_string()));
                Ok(format!("SummaryAgent FAILED — {err}"))
            }
        }
    }
}

pub struct JudgeRigTool {
    pub ctx: Arc<YazgSpecialistContext>,
    pub orchestrator: Arc<dyn PlannerLlm>,
    pub state: SharedYazgRigState,
}

impl Tool for JudgeRigTool {
    const NAME: &'static str = "judge";
    type Error = YazgToolError;
    type Args = ThoughtArgs;
    type Output = String;

    fn description(&self) -> String {
        "Consensus judging via JudgeCoordinatorAgent (JudgeWorker + ClassifierWorker + \
         AttackerWorker) orchestrated by Rig. Requires judge_ready=true and probe/response context."
            .into()
    }

    fn parameters(&self) -> serde_json::Value {
        thought_params()
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let (Some(judge_request), Some(engine)) =
            (self.ctx.judge_request.as_ref(), self.ctx.judge_engine.as_ref())
        else {
            return Ok(
                "JudgeCoordinatorAgent FAILED — judge request/engine not provided".into(),
            );
        };
        {
            let mut guard = self.state.lock().await;
            guard.artifacts.events.push(AgentEvent::info(
                AgentId::Yazg,
                "Acting: JudgeCoordinatorAgent (Rig AgentBuilder)",
            ));
        }
        mark_tool(&self.state, Self::NAME, crate::artifacts::YazgActionKind::Judge).await;
        match JudgeCoordinatorAgent::run_with_orchestrator(
            judge_request,
            engine.clone(),
            self.orchestrator.clone(),
        )
        .await
        {
            Ok(out) => {
                let msg = format!(
                    "JudgeCoordinatorAgent OK — verdict={} confidence={:.2} votes={}",
                    out.verdict.verdict,
                    out.verdict.confidence,
                    out.worker_results.len()
                );
                let mut guard = self.state.lock().await;
                guard.artifacts.events.extend(out.events.clone());
                guard.artifacts.judge = Some(out);
                Ok(msg)
            }
            Err(err) => {
                let mut guard = self.state.lock().await;
                guard.artifacts.events.push(AgentEvent::failed(
                    AgentId::JudgeCoordinator,
                    err.to_string(),
                ));
                Ok(format!("JudgeCoordinatorAgent FAILED — {err}"))
            }
        }
    }
}

/// Helper used by runtime to seed inventory into artifacts without re-query.
pub fn inventory_snapshot(state: &YazgRigRunState) -> Option<&WorkspaceInventory> {
    state.artifacts.workspace_inventory.as_ref()
}
