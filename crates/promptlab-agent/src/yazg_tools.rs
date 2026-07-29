//! Domain tools bound on the Yazg manager (workspace CRUD + specialists).

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
use crate::list_workspace::{clamp_findings_limit, WorkspaceInventory, WorkspaceTools};
use crate::artifacts::YazgArtifacts;
use crate::recommend::RecommendAgent;
use crate::summary::{SummaryAgent, SummaryRequest};
use crate::tool_result::{ToolResult, TOOL_RESULT_CONTRACT};
use crate::types::{AgentEvent, AgentId};

/// Appended to workspace tool descriptions (Prompting Guide: when-to-use lives in tool defs).
const WHEN_NOT_WORKSPACE: &str = " Do not call when the latest user message needs no live workspace \
data (conversation, identity, general knowledge, or simple reasoning without DB rows).";

/// Shared mutable run state filled by tools and consumed after `agent.prompt`.
#[derive(Default)]
pub struct YazgRunState {
    pub artifacts: YazgArtifacts,
    pub last_tool: Option<String>,
    /// Last workspace tool observation text (for empty-reply salvage).
    pub last_workspace_observation: Option<String>,
    /// Accumulated workspace Observations this run (ReAct evidence trail).
    pub workspace_observations: Vec<String>,
}

pub type SharedYazgState = Arc<Mutex<YazgRunState>>;

/// Owned specialist inputs for one Yazg turn (wizard / chat with bound context).
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

/// Arc LLM handles for Yazg supervisor + specialists.
#[derive(Clone)]
pub struct YazgLlms {
    pub supervisor: Arc<dyn PlannerLlm>,
    pub analyze: Arc<dyn PlannerLlm>,
    pub plan: Arc<dyn PlannerLlm>,
    pub prompt: Arc<dyn PlannerLlm>,
    pub recommend: Arc<dyn PlannerLlm>,
    pub summary: Arc<dyn PlannerLlm>,
}

impl YazgLlms {
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

/// True when a host/repo error is a closed-domain miss (entity absent), not a hard failure.
fn is_lookup_miss(err: &str) -> bool {
    let e = err.to_lowercase();
    e.contains("not found")
        || e.contains("no finding")
        || e.contains("no rows")
        || e.contains("does not exist")
}

fn id_name(id: &str, name: &str) -> serde_json::Value {
    json!({ "id": id, "name": name })
}

async fn candidate_projects(tools: &dyn WorkspaceTools) -> Vec<serde_json::Value> {
    match tools.list_workspace().await {
        Ok(inv) => inv
            .projects
            .iter()
            .map(|p| id_name(&p.id, &p.name))
            .collect(),
        Err(_) => Vec::new(),
    }
}

async fn candidate_targets(tools: &dyn WorkspaceTools, project: &str) -> Vec<serde_json::Value> {
    match tools.list_targets(project).await {
        Ok(list) => list
            .targets
            .iter()
            .map(|t| id_name(&t.id, &t.name))
            .collect(),
        Err(_) => Vec::new(),
    }
}

async fn candidate_scans(tools: &dyn WorkspaceTools, project: &str) -> Vec<serde_json::Value> {
    match tools.list_scan(project).await {
        Ok(list) => list
            .scans
            .iter()
            .map(|s| id_name(&s.id, &s.name))
            .collect(),
        Err(_) => Vec::new(),
    }
}

async fn candidate_reports(
    tools: &dyn WorkspaceTools,
    project: Option<&str>,
) -> Vec<serde_json::Value> {
    match tools.list_reports(project).await {
        Ok(list) => list
            .reports
            .iter()
            .map(|r| id_name(&r.id, &r.name))
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn not_found_project_hints() -> Vec<String> {
    vec![
        "List candidates to the user; do not invent or rename another project".into(),
        "Call list_workspace if you need a fresh inventory".into(),
    ]
}

async fn mark_tool(state: &SharedYazgState, name: &str, action: crate::artifacts::YazgActionKind) {
    let mut guard = state.lock().await;
    guard.last_tool = Some(name.into());
    guard.artifacts.last_action = Some(action);
}

pub struct ListWorkspaceTool {
    pub tools: Arc<dyn WorkspaceTools>,
    pub state: SharedYazgState,
}

async fn record_workspace_tool(
    state: &SharedYazgState,
    tool_name: &str,
    action: crate::artifacts::YazgActionKind,
    observation: String,
    event_msg: String,
    inventory: Option<WorkspaceInventory>,
    args_json: &str,
) {
    mark_tool(state, tool_name, action).await;
    let mut guard = state.lock().await;
    let args_snip = truncate(args_json, 160);
    let obs_snip = truncate(&observation, 240);
    guard.artifacts.events.push(AgentEvent::info(
        AgentId::Yazg,
        format!("Acting: {tool_name} args={args_snip}"),
    ));
    guard.artifacts.events.push(AgentEvent::completed(
        AgentId::ListWorkspace,
        format!("{event_msg} | obs={obs_snip}"),
    ));
    guard.workspace_observations.push(observation.clone());
    guard.last_workspace_observation = Some(observation);
    if let Some(inv) = inventory {
        guard.artifacts.workspace_inventory = Some(inv);
    }
}

fn args_json<T: Serialize>(args: &T) -> String {
    serde_json::to_string(args).unwrap_or_else(|_| "{}".into())
}

impl Tool for ListWorkspaceTool {
    const NAME: &'static str = "list_workspace";
    type Error = YazgToolError;
    type Args = EmptyArgs;
    type Output = String;

    fn description(&self) -> String {
        format!(
            "List all projects with aggregate counts only (targets/scans/findings totals). \
             Use when the user wants a workspace inventory / which projects exist. \
             Do not use for one named project's overview (project_detail), targets \
             (list_targets), findings, scans, or reports.{TOOL_RESULT_CONTRACT}\
             {WHEN_NOT_WORKSPACE}"
        )
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
        let observation = ToolResult::ok("list_workspace", &inventory).to_json_string();
        record_workspace_tool(
            &self.state,
            "list_workspace",
            crate::artifacts::YazgActionKind::ListWorkspace,
            observation.clone(),
            format!(
                "Listed {} projects ({} findings total)",
                inventory.totals.projects, inventory.totals.findings
            ),
            Some(inventory),
            "{}",
        )
        .await;
        Ok(observation)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProjectRefArgs {
    /// Project id or name.
    pub project: String,
}

pub struct ProjectDetailTool {
    pub tools: Arc<dyn WorkspaceTools>,
    pub state: SharedYazgState,
}

impl Tool for ProjectDetailTool {
    const NAME: &'static str = "project_detail";
    type Error = YazgToolError;
    type Args = ProjectRefArgs;
    type Output = String;

    fn description(&self) -> String {
        format!(
            "Return one project's metadata, targets, and scans (no finding rows). \
             Use when the user asks about a specific project by exact id or name. \
             Prefer this over list_workspace for named-project questions. \
             `project` must be a real workspace project id/name — not the assistant's name. \
             On miss: status=error error_class=not_found with candidates[]. Do not invent.\
             {TOOL_RESULT_CONTRACT}{WHEN_NOT_WORKSPACE}"
        )
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "project": {
                    "type": "string",
                    "description": "Exact project id or name"
                }
            },
            "required": ["project"],
            "additionalProperties": false
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let requested = args.project.trim();
        if requested.eq_ignore_ascii_case("yazg") {
            let observation = ToolResult::skipped(
                "project_detail",
                "`yazg` is the assistant name, not a workspace project",
                vec![
                    "If the user greeted you or asked who you are, reply in natural language".into(),
                    "Do not invent a project named Yazg".into(),
                ],
            )
            .to_json_string();
            record_workspace_tool(
                &self.state,
                "project_detail",
                crate::artifacts::YazgActionKind::ProjectDetail,
                observation.clone(),
                "Skipped project_detail: yazg is the assistant name".into(),
                None,
                &args_json(&args),
            )
            .await;
            return Ok(observation);
        }
        match self.tools.project_detail(&args.project).await {
            Ok(detail) => {
                let observation = ToolResult::ok("project_detail", &detail).to_json_string();
                record_workspace_tool(
                    &self.state,
                    "project_detail",
                    crate::artifacts::YazgActionKind::ProjectDetail,
                    observation.clone(),
                    format!(
                        "Project detail: {} ({} findings)",
                        detail.project.name, detail.project.findings_count
                    ),
                    None,
                    &args_json(&args),
                )
                .await;
                Ok(observation)
            }
            Err(err) if is_lookup_miss(&err) => {
                let candidates = candidate_projects(self.tools.as_ref()).await;
                let observation = ToolResult::not_found(
                    "project_detail",
                    format!("No project matching `{requested}`"),
                    candidates,
                    not_found_project_hints(),
                )
                .to_json_string();
                record_workspace_tool(
                    &self.state,
                    "project_detail",
                    crate::artifacts::YazgActionKind::ProjectDetail,
                    observation.clone(),
                    format!("Project not found: {requested}"),
                    None,
                    &args_json(&args),
                )
                .await;
                Ok(observation)
            }
            Err(err) => Err(YazgToolError(err)),
        }
    }
}

pub struct ListScanTool {
    pub tools: Arc<dyn WorkspaceTools>,
    pub state: SharedYazgState,
}

impl Tool for ListScanTool {
    const NAME: &'static str = "list_scan";
    type Error = YazgToolError;
    type Args = ProjectRefArgs;
    type Output = String;

    fn description(&self) -> String {
        format!(
            "List scans for a project (exact id or name). Use when the user asks about scans/runs. \
             Not for targets (list_targets) or a full project overview (project_detail). \
             Use scan_detail for one scan. On miss: status=error error_class=not_found with candidates[].\
             {TOOL_RESULT_CONTRACT}{WHEN_NOT_WORKSPACE}"
        )
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "project": {
                    "type": "string",
                    "description": "Project id or name"
                }
            },
            "required": ["project"],
            "additionalProperties": false
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        match self.tools.list_scan(&args.project).await {
            Ok(list) => {
                let observation = ToolResult::ok("list_scan", &list).to_json_string();
                record_workspace_tool(
                    &self.state,
                    "list_scan",
                    crate::artifacts::YazgActionKind::ListScan,
                    observation.clone(),
                    format!(
                        "Listed {} scans for {}",
                        list.scans.len(),
                        list.project_name
                    ),
                    None,
                    &args_json(&args),
                )
                .await;
                Ok(observation)
            }
            Err(err) if is_lookup_miss(&err) => {
                let requested = args.project.trim();
                let project_miss = err.to_lowercase().contains("project not found");
                let candidates = if project_miss {
                    candidate_projects(self.tools.as_ref()).await
                } else {
                    candidate_scans(self.tools.as_ref(), requested).await
                };
                let observation = ToolResult::not_found(
                    "list_scan",
                    format!("No scans for project `{requested}` ({err})"),
                    candidates,
                    not_found_project_hints(),
                )
                .to_json_string();
                record_workspace_tool(
                    &self.state,
                    "list_scan",
                    crate::artifacts::YazgActionKind::ListScan,
                    observation.clone(),
                    format!("list_scan miss: {err}"),
                    None,
                    &args_json(&args),
                )
                .await;
                Ok(observation)
            }
            Err(err) => Err(YazgToolError(err)),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ScanIdArgs {
    pub scan_id: String,
}

pub struct ScanDetailTool {
    pub tools: Arc<dyn WorkspaceTools>,
    pub state: SharedYazgState,
}

impl Tool for ScanDetailTool {
    const NAME: &'static str = "scan_detail";
    type Error = YazgToolError;
    type Args = ScanIdArgs;
    type Output = String;

    fn description(&self) -> String {
        format!(
            "Get one scan by id, including a capped finding list for that scan. \
             Use finding_detail for a single finding. On miss: status=error error_class=not_found.\
             {TOOL_RESULT_CONTRACT}{WHEN_NOT_WORKSPACE}"
        )
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "scan_id": {
                    "type": "string",
                    "description": "Scan id"
                }
            },
            "required": ["scan_id"],
            "additionalProperties": false
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        match self.tools.scan_detail(&args.scan_id).await {
            Ok(detail) => {
                let observation = ToolResult::ok("scan_detail", &detail).to_json_string();
                record_workspace_tool(
                    &self.state,
                    "scan_detail",
                    crate::artifacts::YazgActionKind::ScanDetail,
                    observation.clone(),
                    format!(
                        "Scan detail: {} ({} findings)",
                        detail.scan.name, detail.findings_total
                    ),
                    None,
                    &args_json(&args),
                )
                .await;
                Ok(observation)
            }
            Err(err) if is_lookup_miss(&err) => {
                let observation = ToolResult::not_found(
                    "scan_detail",
                    format!("No scan matching `{}`", args.scan_id.trim()),
                    Vec::new(),
                    vec!["Call list_scan(project) for ids in a known project".into()],
                )
                .to_json_string();
                record_workspace_tool(
                    &self.state,
                    "scan_detail",
                    crate::artifacts::YazgActionKind::ScanDetail,
                    observation.clone(),
                    format!("scan_detail miss: {err}"),
                    None,
                    &args_json(&args),
                )
                .await;
                Ok(observation)
            }
            Err(err) => Err(YazgToolError(err)),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ListFindingsArgs {
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub scan_id: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

pub struct ListFindingsTool {
    pub tools: Arc<dyn WorkspaceTools>,
    pub state: SharedYazgState,
}

impl Tool for ListFindingsTool {
    const NAME: &'static str = "list_findings";
    type Error = YazgToolError;
    type Args = ListFindingsArgs;
    type Output = String;

    fn description(&self) -> String {
        format!(
            "List findings for a project and/or scan (paginated, newest first). \
             Use only when the user asks for findings / vulnerabilities. \
             Provide project (id/name) and/or scan_id. Optional limit (default 20, max 50) and offset. \
             For finding #N use finding_detail(project, index=N). Not for project overview or targets. \
             On miss: status=error error_class=not_found with candidates[].\
             {TOOL_RESULT_CONTRACT}{WHEN_NOT_WORKSPACE}"
        )
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "project": { "type": "string", "description": "Project id or name" },
                "scan_id": { "type": "string", "description": "Scan id" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 50 },
                "offset": { "type": "integer", "minimum": 0 }
            },
            "additionalProperties": false
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        match self
            .tools
            .list_findings(
                args.project.as_deref(),
                args.scan_id.as_deref(),
                clamp_findings_limit(args.limit),
                args.offset.unwrap_or(0),
            )
            .await
        {
            Ok(list) => {
                let observation = ToolResult::ok("list_findings", &list).to_json_string();
                record_workspace_tool(
                    &self.state,
                    "list_findings",
                    crate::artifacts::YazgActionKind::ListFindings,
                    observation.clone(),
                    format!(
                        "Listed {}/{} findings",
                        list.findings.len(),
                        list.total
                    ),
                    None,
                    &args_json(&args),
                )
                .await;
                Ok(observation)
            }
            Err(err) if is_lookup_miss(&err) => {
                let requested = args
                    .scan_id
                    .as_deref()
                    .or(args.project.as_deref())
                    .unwrap_or("(unspecified)")
                    .trim();
                let candidates = candidate_projects(self.tools.as_ref()).await;
                let observation = ToolResult::not_found(
                    "list_findings",
                    format!("No findings for `{requested}` ({err})"),
                    candidates,
                    vec![
                        "Confirm project/scan id, or call list_workspace / list_scan".into(),
                    ],
                )
                .to_json_string();
                record_workspace_tool(
                    &self.state,
                    "list_findings",
                    crate::artifacts::YazgActionKind::ListFindings,
                    observation.clone(),
                    format!("list_findings miss: {err}"),
                    None,
                    &args_json(&args),
                )
                .await;
                Ok(observation)
            }
            Err(err) => Err(YazgToolError(err)),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FindingDetailArgs {
    #[serde(default)]
    pub finding_id: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    /// 1-based index within project findings (newest first).
    #[serde(default)]
    pub index: Option<usize>,
}

pub struct FindingDetailTool {
    pub tools: Arc<dyn WorkspaceTools>,
    pub state: SharedYazgState,
}

impl Tool for FindingDetailTool {
    const NAME: &'static str = "finding_detail";
    type Error = YazgToolError;
    type Args = FindingDetailArgs;
    type Output = String;

    fn description(&self) -> String {
        format!(
            "Get one finding: by finding_id, or by project (id/name) + 1-based index (newest first). \
             Use when the user asks for a specific finding by id or index. \
             On miss: status=error error_class=not_found with candidates[].\
             {TOOL_RESULT_CONTRACT}{WHEN_NOT_WORKSPACE}"
        )
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "finding_id": { "type": "string" },
                "project": { "type": "string", "description": "Project id or name (with index)" },
                "index": { "type": "integer", "minimum": 1, "description": "1-based finding index" }
            },
            "additionalProperties": false
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        match self
            .tools
            .finding_detail(
                args.finding_id.as_deref(),
                args.project.as_deref(),
                args.index,
            )
            .await
        {
            Ok(detail) => {
                let observation = ToolResult::ok("finding_detail", &detail).to_json_string();
                record_workspace_tool(
                    &self.state,
                    "finding_detail",
                    crate::artifacts::YazgActionKind::FindingDetail,
                    observation.clone(),
                    format!("Finding detail: {}", detail.finding.title),
                    None,
                    &args_json(&args),
                )
                .await;
                Ok(observation)
            }
            Err(err) if is_lookup_miss(&err) => {
                let requested = args
                    .finding_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .or_else(|| {
                        args.index.map(|i| {
                            format!(
                                "#{} in {}",
                                i,
                                args.project.as_deref().unwrap_or("?")
                            )
                        })
                    })
                    .unwrap_or_else(|| "(unspecified)".into());
                let project_miss = err.to_lowercase().contains("project not found");
                let candidates = if project_miss {
                    candidate_projects(self.tools.as_ref()).await
                } else {
                    Vec::new()
                };
                let observation = ToolResult::not_found(
                    "finding_detail",
                    format!("No finding matching `{requested}`"),
                    candidates,
                    vec!["Call list_findings(project) for valid ids/indexes".into()],
                )
                .to_json_string();
                record_workspace_tool(
                    &self.state,
                    "finding_detail",
                    crate::artifacts::YazgActionKind::FindingDetail,
                    observation.clone(),
                    format!("finding_detail miss: {err}"),
                    None,
                    &args_json(&args),
                )
                .await;
                Ok(observation)
            }
            Err(err) => Err(YazgToolError(err)),
        }
    }
}

pub struct ListTargetsTool {
    pub tools: Arc<dyn WorkspaceTools>,
    pub state: SharedYazgState,
}

impl Tool for ListTargetsTool {
    const NAME: &'static str = "list_targets";
    type Error = YazgToolError;
    type Args = ProjectRefArgs;
    type Output = String;

    fn description(&self) -> String {
        format!(
            "List AI targets for a project (exact id or name). \
             Use when the user asks for targets / endpoints in a project. \
             Prefer this over list_workspace. Reply from the JSON data; \
             use target_detail only when the user wants one target's profile. \
             On miss: status=error error_class=not_found with candidates[].\
             {TOOL_RESULT_CONTRACT}{WHEN_NOT_WORKSPACE}"
        )
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "project": { "type": "string", "description": "Project id or name" }
            },
            "required": ["project"],
            "additionalProperties": false
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        match self.tools.list_targets(&args.project).await {
            Ok(list) => {
                let observation = ToolResult::ok("list_targets", &list).to_json_string();
                record_workspace_tool(
                    &self.state,
                    "list_targets",
                    crate::artifacts::YazgActionKind::ListTargets,
                    observation.clone(),
                    format!(
                        "Listed {} targets for {}",
                        list.targets.len(),
                        list.project_name
                    ),
                    None,
                    &args_json(&args),
                )
                .await;
                Ok(observation)
            }
            Err(err) if is_lookup_miss(&err) => {
                let requested = args.project.trim();
                let observation = ToolResult::not_found(
                    "list_targets",
                    format!("No project matching `{requested}` for list_targets"),
                    candidate_projects(self.tools.as_ref()).await,
                    not_found_project_hints(),
                )
                .to_json_string();
                record_workspace_tool(
                    &self.state,
                    "list_targets",
                    crate::artifacts::YazgActionKind::ListTargets,
                    observation.clone(),
                    format!("list_targets miss: {err}"),
                    None,
                    &args_json(&args),
                )
                .await;
                Ok(observation)
            }
            Err(err) => Err(YazgToolError(err)),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TargetDetailArgs {
    #[serde(default)]
    pub target_id: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

pub struct TargetDetailTool {
    pub tools: Arc<dyn WorkspaceTools>,
    pub state: SharedYazgState,
}

impl Tool for TargetDetailTool {
    const NAME: &'static str = "target_detail";
    type Error = YazgToolError;
    type Args = TargetDetailArgs;
    type Output = String;

    fn description(&self) -> String {
        format!(
            "Get one target by target_id, or by project + name. \
             Use when the user asks for detail/profile of a specific target. \
             Returns type and clipped profile/descriptor summary. \
             On miss: status=error error_class=not_found with candidates[].\
             {TOOL_RESULT_CONTRACT}{WHEN_NOT_WORKSPACE}"
        )
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "target_id": { "type": "string" },
                "project": { "type": "string", "description": "Project id or name (with name)" },
                "name": { "type": "string", "description": "Target name when resolving via project" }
            },
            "additionalProperties": false
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        match self
            .tools
            .target_detail(
                args.target_id.as_deref(),
                args.project.as_deref(),
                args.name.as_deref(),
            )
            .await
        {
            Ok(detail) => {
                let observation = ToolResult::ok("target_detail", &detail).to_json_string();
                record_workspace_tool(
                    &self.state,
                    "target_detail",
                    crate::artifacts::YazgActionKind::TargetDetail,
                    observation.clone(),
                    format!("Target detail: {}", detail.target.name),
                    None,
                    &args_json(&args),
                )
                .await;
                Ok(observation)
            }
            Err(err) if is_lookup_miss(&err) => {
                let requested = args
                    .target_id
                    .as_deref()
                    .or(args.name.as_deref())
                    .or(args.project.as_deref())
                    .unwrap_or("(unspecified)")
                    .trim();
                let project_miss = err.to_lowercase().contains("project not found");
                let candidates = if project_miss {
                    candidate_projects(self.tools.as_ref()).await
                } else if let Some(project) =
                    args.project.as_deref().map(str::trim).filter(|s| !s.is_empty())
                {
                    candidate_targets(self.tools.as_ref(), project).await
                } else {
                    Vec::new()
                };
                let observation = ToolResult::not_found(
                    "target_detail",
                    format!("No target matching `{requested}`"),
                    candidates,
                    vec!["Call list_targets(project) for valid target names/ids".into()],
                )
                .to_json_string();
                record_workspace_tool(
                    &self.state,
                    "target_detail",
                    crate::artifacts::YazgActionKind::TargetDetail,
                    observation.clone(),
                    format!("target_detail miss: {err}"),
                    None,
                    &args_json(&args),
                )
                .await;
                Ok(observation)
            }
            Err(err) => Err(YazgToolError(err)),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ListReportsArgs {
    #[serde(default)]
    pub project: Option<String>,
}

pub struct ListReportsTool {
    pub tools: Arc<dyn WorkspaceTools>,
    pub state: SharedYazgState,
}

impl Tool for ListReportsTool {
    const NAME: &'static str = "list_reports";
    type Error = YazgToolError;
    type Args = ListReportsArgs;
    type Output = String;

    fn description(&self) -> String {
        format!(
            "List generated reports for a project (id/name), or all projects when project is omitted. \
             Use when the user asks about reports. Use report_detail to read one report. \
             On miss: status=error error_class=not_found with candidates[].\
             {TOOL_RESULT_CONTRACT}{WHEN_NOT_WORKSPACE}"
        )
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "project": { "type": "string", "description": "Optional project id or name" }
            },
            "additionalProperties": false
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        match self.tools.list_reports(args.project.as_deref()).await {
            Ok(list) => {
                let observation = ToolResult::ok("list_reports", &list).to_json_string();
                record_workspace_tool(
                    &self.state,
                    "list_reports",
                    crate::artifacts::YazgActionKind::ListReports,
                    observation.clone(),
                    format!("Listed {} reports", list.reports.len()),
                    None,
                    &args_json(&args),
                )
                .await;
                Ok(observation)
            }
            Err(err) if is_lookup_miss(&err) => {
                let requested = args
                    .project
                    .as_deref()
                    .unwrap_or("(unspecified)")
                    .trim();
                let observation = ToolResult::not_found(
                    "list_reports",
                    format!("No project matching `{requested}` for list_reports"),
                    candidate_projects(self.tools.as_ref()).await,
                    not_found_project_hints(),
                )
                .to_json_string();
                record_workspace_tool(
                    &self.state,
                    "list_reports",
                    crate::artifacts::YazgActionKind::ListReports,
                    observation.clone(),
                    format!("list_reports miss: {err}"),
                    None,
                    &args_json(&args),
                )
                .await;
                Ok(observation)
            }
            Err(err) => Err(YazgToolError(err)),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReportIdArgs {
    pub report_id: String,
}

pub struct ReportDetailTool {
    pub tools: Arc<dyn WorkspaceTools>,
    pub state: SharedYazgState,
}

impl Tool for ReportDetailTool {
    const NAME: &'static str = "report_detail";
    type Error = YazgToolError;
    type Args = ReportIdArgs;
    type Output = String;

    fn description(&self) -> String {
        format!(
            "Read one report by id (metadata + clipped content preview). \
             On miss: status=error error_class=not_found with candidates[].\
             {TOOL_RESULT_CONTRACT}{WHEN_NOT_WORKSPACE}"
        )
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "report_id": { "type": "string" }
            },
            "required": ["report_id"],
            "additionalProperties": false
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        match self.tools.report_detail(&args.report_id).await {
            Ok(detail) => {
                let observation = ToolResult::ok("report_detail", &detail).to_json_string();
                record_workspace_tool(
                    &self.state,
                    "report_detail",
                    crate::artifacts::YazgActionKind::ReportDetail,
                    observation.clone(),
                    format!("Report detail: {}", detail.report.name),
                    None,
                    &args_json(&args),
                )
                .await;
                Ok(observation)
            }
            Err(err) if is_lookup_miss(&err) => {
                let observation = ToolResult::not_found(
                    "report_detail",
                    format!("No report matching `{}`", args.report_id.trim()),
                    candidate_reports(self.tools.as_ref(), None).await,
                    vec!["Call list_reports(project?) for valid report ids".into()],
                )
                .to_json_string();
                record_workspace_tool(
                    &self.state,
                    "report_detail",
                    crate::artifacts::YazgActionKind::ReportDetail,
                    observation.clone(),
                    format!("report_detail miss: {err}"),
                    None,
                    &args_json(&args),
                )
                .await;
                Ok(observation)
            }
            Err(err) => Err(YazgToolError(err)),
        }
    }
}

pub struct CreateProjectTool {
    pub tools: Arc<dyn CreateProjectTools>,
    pub state: SharedYazgState,
}

impl Tool for CreateProjectTool {
    const NAME: &'static str = "create_project";
    type Error = YazgToolError;
    type Args = CreateProjectArgs;
    type Output = String;

    fn description(&self) -> String {
        format!(
            "Create a workspace project in the local database. Requires a project name. \
             Optional description. Do NOT ask for a scan target.{TOOL_RESULT_CONTRACT}"
        )
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
            let observation = ToolResult::validation(
                "create_project",
                "missing project name — provide JSON field \"name\"",
                vec!["Retry with a non-empty name".into()],
            )
            .to_json_string();
            record_workspace_tool(
                &self.state,
                "create_project",
                crate::artifacts::YazgActionKind::CreateProject,
                observation.clone(),
                "create_project validation miss".into(),
                None,
                &args_json(&args),
            )
            .await;
            return Ok(observation);
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
        let created = CreatedProject {
            id: project.id.clone(),
            name: project.name.clone(),
            description: project.description.clone(),
        };
        let observation = ToolResult::ok("create_project", &created).to_json_string();
        record_workspace_tool(
            &self.state,
            "create_project",
            crate::artifacts::YazgActionKind::CreateProject,
            observation.clone(),
            format!("Created project {}", created.name),
            None,
            &args_json(&args),
        )
        .await;
        let mut guard = self.state.lock().await;
        guard.artifacts.events.push(AgentEvent::completed(
            AgentId::CreateProject,
            format!("Created project {}", created.name),
        ));
        guard.artifacts.created_project = Some(created);
        Ok(observation)
    }
}

pub struct AnalyzeEndpointTool {
    pub ctx: Arc<YazgSpecialistContext>,
    pub llm: Arc<dyn PlannerLlm>,
    pub state: SharedYazgState,
}

impl Tool for AnalyzeEndpointTool {
    const NAME: &'static str = "analyze_endpoint";
    type Error = YazgToolError;
    type Args = ThoughtArgs;
    type Output = String;

    fn description(&self) -> String {
        "Probe/classify whether a bound live scan target is a generative AI API \
         (AnalyzeEndpointAgent). Requires a bound target or capability_probe_ready=true \
         (Scan wizard Verification). Do NOT use for greetings, identity, general chat, \
         or workspace inventory questions."
            .into()
    }

    fn parameters(&self) -> serde_json::Value {
        thought_params()
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let Some(profile) = self.ctx.profile.as_ref() else {
            let observation = ToolResult::skipped(
                "analyze_endpoint",
                "No scan target is bound — cannot analyze an endpoint",
                vec![
                    "If the user greeted you or asked a non-endpoint question, reply in natural language".into(),
                    "Only call analyze_endpoint when a live target or capability_probe is available".into(),
                ],
            )
            .to_json_string();
            let mut guard = self.state.lock().await;
            guard.last_tool = Some("analyze_endpoint".into());
            guard.artifacts.last_action =
                Some(crate::artifacts::YazgActionKind::AnalyzeEndpoint);
            guard.artifacts.events.push(AgentEvent::info(
                AgentId::Yazg,
                "analyze_endpoint skipped: no target bound",
            ));
            guard.workspace_observations.push(observation.clone());
            guard.last_workspace_observation = Some(observation.clone());
            return Ok(observation);
        };
        {
            let mut guard = self.state.lock().await;
            guard.artifacts.events.push(AgentEvent::info(
                AgentId::Yazg,
                "Acting: AnalyzeEndpointAgent",
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
            "analyze_endpoint",
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

pub struct AttackPlanTool {
    pub ctx: Arc<YazgSpecialistContext>,
    pub llm: Arc<dyn PlannerLlm>,
    pub state: SharedYazgState,
}

impl Tool for AttackPlanTool {
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
                .push(AgentEvent::info(AgentId::Yazg, "Acting: AttackPlanAgent"));
        }
        mark_tool(
            &self.state,
            "attack_plan",
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

pub struct GeneratePromptTool {
    pub ctx: Arc<YazgSpecialistContext>,
    pub llm: Arc<dyn PlannerLlm>,
    pub state: SharedYazgState,
}

impl Tool for GeneratePromptTool {
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
                "Acting: GeneratePromptAgent",
            ));
        }
        mark_tool(
            &self.state,
            "generate_prompt",
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

pub struct RecommendTool {
    pub ctx: Arc<YazgSpecialistContext>,
    pub llm: Arc<dyn PlannerLlm>,
    pub state: SharedYazgState,
}

impl Tool for RecommendTool {
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
                .push(AgentEvent::info(AgentId::Yazg, "Acting: RecommendAgent"));
        }
        mark_tool(
            &self.state,
            "recommend",
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

pub struct SummaryTool {
    pub ctx: Arc<YazgSpecialistContext>,
    pub llm: Arc<dyn PlannerLlm>,
    pub state: SharedYazgState,
}

impl Tool for SummaryTool {
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
                .push(AgentEvent::info(AgentId::Yazg, "Acting: SummaryAgent"));
        }
        mark_tool(&self.state, "summary", crate::artifacts::YazgActionKind::Summary).await;
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

pub struct JudgeTool {
    pub ctx: Arc<YazgSpecialistContext>,
    pub orchestrator: Arc<dyn PlannerLlm>,
    pub state: SharedYazgState,
}

impl Tool for JudgeTool {
    const NAME: &'static str = "judge";
    type Error = YazgToolError;
    type Args = ThoughtArgs;
    type Output = String;

    fn description(&self) -> String {
        "Consensus judging via JudgeCoordinatorAgent (JudgeWorker + ClassifierWorker + \
         AttackerWorker). Requires judge_ready=true and probe/response context."
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
                "Acting: JudgeCoordinatorAgent",
            ));
        }
        mark_tool(&self.state, "judge", crate::artifacts::YazgActionKind::Judge).await;
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
pub fn inventory_snapshot(state: &YazgRunState) -> Option<&WorkspaceInventory> {
    state.artifacts.workspace_inventory.as_ref()
}
