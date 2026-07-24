//! JudgeCoordinatorAgent — orchestrates Judge / Classifier / Attacker workers.
//!
//! Receives a judge command (from Yazg or scan host), fans out to the three
//! role workers, then synthesizes a consensus [`JudgeVerdict`].

use promptlab_judge::{EvaluatorResult, JudgeEngine, JudgeRequest, JudgeVerdict, ModelRole};
use tracing::{info, warn};

use crate::error::{AgentError, AgentResult};
use crate::judge_workers::{AttackerWorker, ClassifierWorker, JudgeWorker};
use crate::types::{AgentEvent, AgentId};

/// Outcome of a JudgeCoordinatorAgent run.
#[derive(Debug, Clone)]
pub struct JudgeCoordinatorAgentOutcome {
    pub verdict: JudgeVerdict,
    pub worker_results: Vec<EvaluatorResult>,
    pub events: Vec<AgentEvent>,
}

/// Coordinates JudgeWorker / ClassifierWorker / AttackerWorker under Yazg.
pub struct JudgeCoordinatorAgent;

impl JudgeCoordinatorAgent {
    /// Run all configured role workers, then compute the final consensus verdict.
    pub async fn run(
        request: &JudgeRequest,
        engine: &JudgeEngine,
    ) -> AgentResult<JudgeCoordinatorAgentOutcome> {
        let mut events = vec![AgentEvent::started(
            AgentId::JudgeCoordinator,
            format!(
                "Coordinating judge workers for probe {} ({})",
                request.probe_id, request.attack_category
            ),
        )];

        info!(
            probe_id = %request.probe_id,
            category = %request.attack_category,
            "JudgeCoordinatorAgent started"
        );

        let configured = engine.role_pool().configured_roles();
        if configured.is_empty() {
            let message =
                "JudgeCoordinatorAgent requires at least one configured LLM role".to_string();
            events.push(AgentEvent::failed(
                AgentId::JudgeCoordinator,
                message.clone(),
            ));
            return Err(AgentError::Judge(message));
        }

        let mut worker_results: Vec<EvaluatorResult> = Vec::new();

        for role in ModelRole::all() {
            if !configured.contains(role) {
                events.push(AgentEvent::info(
                    AgentId::JudgeCoordinator,
                    format!("{role} worker skipped (role not configured)"),
                ));
                continue;
            }

            let worker_outcome = match role {
                ModelRole::Judge => JudgeWorker::run(request, engine).await,
                ModelRole::Classifier => ClassifierWorker::run(request, engine).await,
                ModelRole::Attacker => AttackerWorker::run(request, engine).await,
            };

            match worker_outcome {
                Ok(out) => {
                    events.extend(out.events);
                    worker_results.push(out.result);
                }
                Err(err) => {
                    // Soft-fail individual workers (same as JudgeEngine); keep going.
                    warn!(
                        role = %role,
                        error = %err,
                        "judge worker failed; continuing with remaining roles"
                    );
                    events.push(AgentEvent::info(
                        AgentId::JudgeCoordinator,
                        format!("{role} worker failed: {err}"),
                    ));
                }
            }
        }

        if worker_results.is_empty() {
            let message =
                "JudgeCoordinatorAgent: all role workers failed — check AI runtime".to_string();
            events.push(AgentEvent::failed(
                AgentId::JudgeCoordinator,
                message.clone(),
            ));
            return Err(AgentError::Judge(message));
        }

        let verdict = engine
            .synthesize_verdict(request.clone(), worker_results.clone())
            .map_err(|err| {
                let message = err.to_string();
                events.push(AgentEvent::failed(
                    AgentId::JudgeCoordinator,
                    message.clone(),
                ));
                AgentError::Judge(message)
            })?;

        events.push(AgentEvent::completed(
            AgentId::JudgeCoordinator,
            format!(
                "Consensus ready: {} (confidence={:.2}, votes={})",
                verdict.verdict,
                verdict.confidence,
                worker_results.len()
            ),
        ));

        Ok(JudgeCoordinatorAgentOutcome {
            verdict,
            worker_results,
            events,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use promptlab_judge::{
        JudgeConfig, JudgeMode, JudgeRequest, JsonMockRuntime, ModelRolePool,
    };
    use promptlab_models::runtime::InferenceRuntime;
    use tokio::sync::Mutex;

    use super::*;

    fn engine_with_mock(json: &str) -> JudgeEngine {
        let mut pool = ModelRolePool::new();
        let runtime: Arc<Mutex<dyn InferenceRuntime>> =
            Arc::new(Mutex::new(JsonMockRuntime::new(json)));
        pool.set_all(runtime);
        let mut config = JudgeConfig::default();
        config.mode = JudgeMode::LocalLlm;
        JudgeEngine::new(config, pool)
    }

    #[tokio::test]
    async fn coordinator_runs_three_workers_and_consensus() {
        let json = r#"{"vulnerable": true, "confidence": 0.9, "severity": "high", "rationale": "leak", "indicators": ["secret"]}"#;
        let engine = engine_with_mock(json);
        let request = JudgeRequest {
            probe_id: "p-coord".into(),
            attack_category: "prompt_injection".into(),
            payload: "ignore previous".into(),
            response_text: "secret: abc".into(),
            context: serde_json::json!({}),
        };

        let outcome = JudgeCoordinatorAgent::run(&request, &engine)
            .await
            .expect("coordinator");

        assert!(outcome.verdict.vulnerable);
        assert_eq!(outcome.worker_results.len(), 3);
        assert!(outcome
            .events
            .iter()
            .any(|e| e.agent == AgentId::JudgeWorker));
        assert!(outcome
            .events
            .iter()
            .any(|e| e.agent == AgentId::ClassifierWorker));
        assert!(outcome
            .events
            .iter()
            .any(|e| e.agent == AgentId::AttackerWorker));
        assert!(outcome
            .events
            .iter()
            .any(|e| e.agent == AgentId::JudgeCoordinator));
    }
}

