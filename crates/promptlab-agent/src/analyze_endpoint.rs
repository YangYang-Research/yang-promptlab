//! AnalyzeEndpointAgent — sub-agent for endpoint / AI capability analysis.
//!
//! Kind **B-ish**: network HTTP probe + LLM classification (not prompt-only).

use std::collections::HashMap;

use promptlab_planner::PlannerLlm;
use promptlab_target_profile::{
    validate_http_response_with_llm, verify_target_profile_with_llm, TargetProfile,
    VerificationAttempt, VerificationConsoleEntry, VerificationResult, VerifyHttpSuccess,
};
use tracing::info;

use crate::error::{AgentError, AgentResult};
use crate::types::{AgentEvent, AgentId};

/// Outcome of an AnalyzeEndpointAgent run (success path).
#[derive(Debug, Clone)]
pub struct AnalyzeEndpointAgentOutcome {
    pub verification: VerificationResult,
    pub console: VerificationConsoleEntry,
    pub events: Vec<AgentEvent>,
}

/// Endpoint analysis sub-agent under Yazg.
pub struct AnalyzeEndpointAgent;

impl AnalyzeEndpointAgent {
    /// Full analysis: capability HTTP probe + Yazg LLM classification.
    pub async fn run(
        profile: &TargetProfile,
        auth_headers: HashMap<String, String>,
        llm: &dyn PlannerLlm,
    ) -> AgentResult<AnalyzeEndpointAgentOutcome> {
        promptlab_agenttrace::trace_agent_run(
            AgentId::AnalyzeEndpoint.as_str(),
            Self::run_traced(profile, auth_headers, llm),
        )
        .await
    }

    async fn run_traced(
        profile: &TargetProfile,
        auth_headers: HashMap<String, String>,
        llm: &dyn PlannerLlm,
    ) -> AgentResult<AnalyzeEndpointAgentOutcome> {
        let mut events = vec![AgentEvent::started(
            AgentId::AnalyzeEndpoint,
            format!("Analyzing endpoint {}", profile.full_url()),
        )];

        info!(endpoint = %profile.full_url(), "AnalyzeEndpointAgent started");
        let reflecting =
            crate::agent_reflection::ReflectingLlm::new(AgentId::AnalyzeEndpoint, llm);
        let llm: &dyn PlannerLlm = &reflecting;
        let attempt = verify_target_profile_with_llm(profile, auth_headers, llm).await;
        map_attempt(attempt, &mut events)
    }

    /// Classify an already-successful capability probe (wizard step 2).
    pub async fn classify_probe(
        profile: &TargetProfile,
        http: &VerifyHttpSuccess,
        llm: &dyn PlannerLlm,
    ) -> AgentResult<AnalyzeEndpointAgentOutcome> {
        promptlab_agenttrace::trace_agent_run(
            AgentId::AnalyzeEndpoint.as_str(),
            Self::classify_probe_traced(profile, http, llm),
        )
        .await
    }

    async fn classify_probe_traced(
        profile: &TargetProfile,
        http: &VerifyHttpSuccess,
        llm: &dyn PlannerLlm,
    ) -> AgentResult<AnalyzeEndpointAgentOutcome> {
        let mut events = vec![AgentEvent::started(
            AgentId::AnalyzeEndpoint,
            "Classifying capability probe with Yazg",
        )];

        let reflecting =
            crate::agent_reflection::ReflectingLlm::new(AgentId::AnalyzeEndpoint, llm);
        let llm: &dyn PlannerLlm = &reflecting;
        let attempt = validate_http_response_with_llm(profile, http, llm).await;
        map_attempt(attempt, &mut events)
    }
}

fn map_attempt(
    attempt: VerificationAttempt,
    events: &mut Vec<AgentEvent>,
) -> AgentResult<AnalyzeEndpointAgentOutcome> {
    match attempt.result {
        Ok(verification) => {
            events.push(AgentEvent::completed(
                AgentId::AnalyzeEndpoint,
                format!(
                    "Analyzed as AI endpoint (status {}, {} ms)",
                    verification.status_code, verification.response_time_ms
                ),
            ));
            Ok(AnalyzeEndpointAgentOutcome {
                verification,
                console: attempt.console,
                events: std::mem::take(events),
            })
        }
        Err(err) => {
            let message = err.to_string();
            events.push(AgentEvent::failed(AgentId::AnalyzeEndpoint, message.clone()));
            Err(AgentError::AnalyzeEndpoint(message))
        }
    }
}
