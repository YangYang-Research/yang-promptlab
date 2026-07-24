//! RecommendAgent — post-scan remediation recommendations.
//!
//! Kind **A**: LLM remediation guidance from an attack-results summary.

use promptlab_planner::PlannerLlm;
use promptlab_target_profile::{
    ensure_failed_scan_action_recommendation, generate_attack_recommendations_with_llm,
    AttackRecommendationsBundle, AttackResultsSummary,
};
use tracing::info;

use crate::error::{AgentError, AgentResult};
use crate::types::{AgentEvent, AgentId};

/// Outcome of a RecommendAgent run.
#[derive(Debug, Clone)]
pub struct RecommendAgentOutcome {
    pub bundle: AttackRecommendationsBundle,
    pub events: Vec<AgentEvent>,
}

/// Post-scan recommendations sub-agent under Yazg.
pub struct RecommendAgent;

impl RecommendAgent {
    /// Produce overview + prioritized remediation recommendations.
    pub async fn run(
        summary: &AttackResultsSummary,
        llm: &dyn PlannerLlm,
    ) -> AgentResult<RecommendAgentOutcome> {
        let mut events = vec![AgentEvent::started(
            AgentId::Recommend,
            format!(
                "Generating recommendations ({} findings, status={})",
                summary.total_findings, summary.scan_status
            ),
        )];

        info!(
            findings = summary.total_findings,
            status = %summary.scan_status,
            "RecommendAgent started"
        );

        match generate_attack_recommendations_with_llm(summary, llm).await {
            Ok(bundle) => {
                let bundle = ensure_failed_scan_action_recommendation(summary, bundle);
                events.push(AgentEvent::completed(
                    AgentId::Recommend,
                    format!(
                        "Recommendations ready ({} items)",
                        bundle.recommendations.len()
                    ),
                ));
                Ok(RecommendAgentOutcome { bundle, events })
            }
            Err(err) => {
                let message = err.to_string();
                events.push(AgentEvent::failed(AgentId::Recommend, message.clone()));
                Err(AgentError::Recommend(message))
            }
        }
    }
}
