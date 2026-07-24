//! Role workers under JudgeCoordinatorAgent.
//!
//! Each worker runs one [`promptlab_judge::ModelRole`] evaluator and returns a vote.

use promptlab_judge::{EvaluatorResult, JudgeEngine, JudgeRequest, ModelRole};
use tracing::info;

use crate::error::{AgentError, AgentResult};
use crate::types::{AgentEvent, AgentId};

/// Outcome of a single role worker.
#[derive(Debug, Clone)]
pub struct JudgeWorkerOutcome {
    pub role: ModelRole,
    pub result: EvaluatorResult,
    pub events: Vec<AgentEvent>,
}

fn agent_id_for_role(role: ModelRole) -> AgentId {
    match role {
        ModelRole::Judge => AgentId::JudgeWorker,
        ModelRole::Classifier => AgentId::ClassifierWorker,
        ModelRole::Attacker => AgentId::AttackerWorker,
    }
}

async fn run_role_worker(
    role: ModelRole,
    request: &JudgeRequest,
    engine: &JudgeEngine,
) -> AgentResult<JudgeWorkerOutcome> {
    let agent = agent_id_for_role(role);
    let mut events = vec![AgentEvent::started(
        agent,
        format!("Evaluating as {role} for probe {}", request.probe_id),
    )];

    info!(
        role = %role,
        probe_id = %request.probe_id,
        "judge role worker started"
    );

    match engine.evaluate_role(role, request).await {
        Ok(result) => {
            events.push(AgentEvent::completed(
                agent,
                format!(
                    "{role} vote: vulnerable={} confidence={:.2}",
                    result.vulnerable, result.confidence
                ),
            ));
            Ok(JudgeWorkerOutcome {
                role,
                result,
                events,
            })
        }
        Err(err) => {
            let message = err.to_string();
            events.push(AgentEvent::failed(agent, message.clone()));
            Err(AgentError::Judge(format!("{role} worker failed: {message}")))
        }
    }
}

/// Judge role worker — success / vulnerability decision.
pub struct JudgeWorker;

impl JudgeWorker {
    pub async fn run(
        request: &JudgeRequest,
        engine: &JudgeEngine,
    ) -> AgentResult<JudgeWorkerOutcome> {
        run_role_worker(ModelRole::Judge, request, engine).await
    }
}

/// Classifier role worker — category + severity.
pub struct ClassifierWorker;

impl ClassifierWorker {
    pub async fn run(
        request: &JudgeRequest,
        engine: &JudgeEngine,
    ) -> AgentResult<JudgeWorkerOutcome> {
        run_role_worker(ModelRole::Classifier, request, engine).await
    }
}

/// Attacker role worker — adversarial compliance with attack intent.
pub struct AttackerWorker;

impl AttackerWorker {
    pub async fn run(
        request: &JudgeRequest,
        engine: &JudgeEngine,
    ) -> AgentResult<JudgeWorkerOutcome> {
        run_role_worker(ModelRole::Attacker, request, engine).await
    }
}
