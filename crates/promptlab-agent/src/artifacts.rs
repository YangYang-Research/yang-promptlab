//! Shared Yazg run artifacts (used by Rig supervisor runtime).

use crate::analyze_endpoint::AnalyzeEndpointAgentOutcome;
use crate::attack_plan::AttackPlanAgentOutcome;
use crate::create_project::CreatedProject;
use crate::generate_prompt::GeneratePromptAgentOutcome;
use crate::judge_coordinator::JudgeCoordinatorAgentOutcome;
use crate::list_workspace::WorkspaceInventory;
use crate::memory::{
    remember_ltm, AgentMemoryStore, LtmWrite, MemoryContext, MemoryScopeType,
};
use crate::recommend::RecommendAgentOutcome;
use crate::summary::SummaryAgentOutcome;
use crate::types::{AgentEvent, AgentId};

/// Last specialist / workspace action chosen in a Yazg turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YazgActionKind {
    AnalyzeEndpoint,
    AttackPlan,
    GeneratePrompt,
    Recommend,
    Summary,
    Judge,
    CreateProject,
    ListWorkspace,
    ProjectDetail,
    ListScan,
    ScanDetail,
    ListFindings,
    FindingDetail,
    Finish,
}

/// Typed outputs collected during a Yazg turn.
#[derive(Debug, Clone, Default)]
pub struct YazgArtifacts {
    pub events: Vec<AgentEvent>,
    pub analyze: Option<AnalyzeEndpointAgentOutcome>,
    pub plan: Option<AttackPlanAgentOutcome>,
    pub generate_prompt: Option<GeneratePromptAgentOutcome>,
    pub recommend: Option<RecommendAgentOutcome>,
    pub summary: Option<SummaryAgentOutcome>,
    pub judge: Option<JudgeCoordinatorAgentOutcome>,
    pub created_project: Option<CreatedProject>,
    pub workspace_inventory: Option<WorkspaceInventory>,
    pub final_reply: String,
    pub last_action: Option<YazgActionKind>,
}

/// Persist specialist outcomes into LTM after a Yazg turn.
pub async fn persist_artifacts_ltm(
    memory: Option<&dyn AgentMemoryStore>,
    memory_ctx: &MemoryContext,
    artifacts: &YazgArtifacts,
) {
    let (scope_type, scope_id) = memory_ctx.primary_scope();

    if let Some(analyze) = artifacts.analyze.as_ref() {
        remember_ltm(
            memory,
            LtmWrite {
                agent_id: AgentId::AnalyzeEndpoint,
                scope_type,
                scope_id: scope_id.clone(),
                memory_key: "target.verification".into(),
                content: format!(
                    "verified status={} provider={} model={}",
                    analyze.verification.status_code,
                    analyze.verification.provider,
                    analyze.verification.model.as_deref().unwrap_or("unknown")
                ),
                content_json: Some(serde_json::json!({
                    "status_code": analyze.verification.status_code,
                    "provider": analyze.verification.provider,
                    "model": analyze.verification.model,
                })),
                importance: 0.85,
            },
        )
        .await;
    }

    if let Some(plan) = artifacts.plan.as_ref() {
        remember_ltm(
            memory,
            LtmWrite {
                agent_id: AgentId::AttackPlan,
                scope_type,
                scope_id: scope_id.clone(),
                memory_key: "target.attack_plan".into(),
                content: format!(
                    "profile={} categories={} source={}",
                    plan.plan.profile_id,
                    plan.plan.categories.len(),
                    plan.plan.planner_source
                ),
                content_json: Some(serde_json::json!({
                    "profile_id": plan.plan.profile_id,
                    "recommended_profile_id": plan.plan.recommended_profile_id,
                    "categories": plan.plan.categories.iter().map(|c| c.as_str()).collect::<Vec<_>>(),
                    "planner_source": plan.plan.planner_source,
                })),
                importance: 0.9,
            },
        )
        .await;
    }

    if let Some(recommend) = artifacts.recommend.as_ref() {
        let scope = if let Some(scan_id) = memory_ctx.scan_id.as_ref() {
            (MemoryScopeType::Scan, scan_id.clone())
        } else {
            (scope_type, scope_id.clone())
        };
        remember_ltm(
            memory,
            LtmWrite {
                agent_id: AgentId::Recommend,
                scope_type: scope.0,
                scope_id: scope.1,
                memory_key: "scan.recommendations".into(),
                content: truncate(&recommend.bundle.overview, 400),
                content_json: Some(serde_json::json!({
                    "overview": recommend.bundle.overview,
                    "count": recommend.bundle.recommendations.len(),
                })),
                importance: 0.8,
            },
        )
        .await;
    }

    if let Some(summary) = artifacts.summary.as_ref() {
        let (st, sid) = match summary.kind.as_str() {
            "project" => {
                if let Some(project_id) = memory_ctx.project_id.as_ref() {
                    (MemoryScopeType::Project, project_id.clone())
                } else {
                    (scope_type, scope_id.clone())
                }
            }
            "scan" => {
                if let Some(scan_id) = memory_ctx.scan_id.as_ref() {
                    (MemoryScopeType::Scan, scan_id.clone())
                } else {
                    (scope_type, scope_id.clone())
                }
            }
            _ => (scope_type, scope_id.clone()),
        };
        remember_ltm(
            memory,
            LtmWrite {
                agent_id: AgentId::Summary,
                scope_type: st,
                scope_id: sid,
                memory_key: format!("summary.{}", summary.kind),
                content: truncate(&summary.bundle.overview, 400),
                content_json: Some(serde_json::json!({
                    "kind": summary.kind,
                    "overview": summary.bundle.overview,
                    "highlights": summary.bundle.highlights.len(),
                })),
                importance: 0.75,
            },
        )
        .await;
    }

    if let Some(gen) = artifacts.generate_prompt.as_ref() {
        remember_ltm(
            memory,
            LtmWrite {
                agent_id: AgentId::GeneratePrompt,
                scope_type: MemoryScopeType::Global,
                scope_id: String::new(),
                memory_key: format!("factory.{}", gen.technique_id),
                content: truncate(&gen.content, 400),
                content_json: Some(serde_json::json!({
                    "technique_id": gen.technique_id,
                    "chars": gen.content.chars().count(),
                })),
                importance: 0.65,
            },
        )
        .await;
    }

    if let Some(judge) = artifacts.judge.as_ref() {
        remember_ltm(
            memory,
            LtmWrite {
                agent_id: AgentId::JudgeCoordinator,
                scope_type,
                scope_id,
                memory_key: "judge.last_verdict".into(),
                content: format!(
                    "vulnerable={} confidence={:.2}",
                    judge.verdict.vulnerable, judge.verdict.confidence
                ),
                content_json: Some(serde_json::json!({
                    "vulnerable": judge.verdict.vulnerable,
                    "confidence": judge.verdict.confidence,
                })),
                importance: 0.7,
            },
        )
        .await;
    }
}

fn truncate(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    let shortened: String = t.chars().take(max.saturating_sub(1)).collect();
    format!("{shortened}…")
}
