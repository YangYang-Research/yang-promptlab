//! SummaryAgent — project / scan posture summaries.
//!
//! Kind **A**: LLM overview + highlights for project or scan scope.

use aisec_planner::PlannerLlm;
use aisec_target_profile::{
    generate_project_summary_with_llm, generate_scan_summary_with_llm, AttackResultsSummary,
    SummaryBundle,
};
use tracing::info;

use crate::error::{AgentError, AgentResult};
use crate::types::{AgentEvent, AgentId};

/// Which summary SummaryAgent should produce.
#[derive(Debug, Clone)]
pub enum SummaryRequest {
    /// Host-built project summary input JSON.
    Project {
        project_name: String,
        input_json: String,
    },
    /// Attack-results summary for a single scan.
    Scan { summary: AttackResultsSummary },
}

impl SummaryRequest {
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Project { .. } => "project",
            Self::Scan { .. } => "scan",
        }
    }
}

/// Outcome of a SummaryAgent run.
#[derive(Debug, Clone)]
pub struct SummaryAgentOutcome {
    pub kind: String,
    pub bundle: SummaryBundle,
    pub events: Vec<AgentEvent>,
}

/// Project / scan summary sub-agent under Yazg.
pub struct SummaryAgent;

impl SummaryAgent {
    /// Produce overview + highlights for project or scan scope.
    pub async fn run(
        request: &SummaryRequest,
        llm: &dyn PlannerLlm,
    ) -> AgentResult<SummaryAgentOutcome> {
        let kind = request.kind_label().to_string();
        let mut events = vec![AgentEvent::started(
            AgentId::Summary,
            format!("Generating {kind} summary"),
        )];

        info!(kind = %kind, "SummaryAgent started");

        let result = match request {
            SummaryRequest::Project { input_json, .. } => {
                generate_project_summary_with_llm(input_json, llm).await
            }
            SummaryRequest::Scan { summary } => {
                generate_scan_summary_with_llm(summary, llm).await
            }
        };

        match result {
            Ok(bundle) => {
                events.push(AgentEvent::completed(
                    AgentId::Summary,
                    format!(
                        "{kind} summary ready ({} highlights)",
                        bundle.highlights.len()
                    ),
                ));
                Ok(SummaryAgentOutcome {
                    kind,
                    bundle,
                    events,
                })
            }
            Err(err) => {
                let message = err.to_string();
                events.push(AgentEvent::failed(AgentId::Summary, message.clone()));
                Err(AgentError::Summary(message))
            }
        }
    }
}
