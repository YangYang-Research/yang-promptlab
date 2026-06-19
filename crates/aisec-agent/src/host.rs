use async_trait::async_trait;

use aisec_attack::AttackCategory;
use aisec_generator::{GeneratorMode, PromptPayloads};
use aisec_planner::{AttackPlan, FingerprintResult};
use aisec_fingerprint::StackFingerprintReport;

use crate::error::AgentResult;
use crate::types::{AgentPhase, AttackExecutionSummary};

/// Bridge for attack execution, judging, and persistence (implemented by Tauri).
#[async_trait]
pub trait AgentHost: Send {
    async fn is_cancelled(&self) -> bool;

    async fn load_fingerprint(
        &self,
        endpoint_id: &str,
        url: &str,
    ) -> AgentResult<StackFingerprintReport>;

    async fn on_phase(
        &mut self,
        phase: AgentPhase,
        detail: &str,
        attempt: u32,
        retry: u32,
    );

    async fn plan(&mut self, fingerprint: &FingerprintResult) -> AgentResult<AttackPlan>;

    async fn generate_payloads(
        &mut self,
        plan: &AttackPlan,
        category: AttackCategory,
        mode: GeneratorMode,
    ) -> AgentResult<PromptPayloads>;

    async fn execute_attack(
        &mut self,
        category: AttackCategory,
        payloads: &PromptPayloads,
    ) -> AgentResult<AttackExecutionSummary>;

    async fn evaluate_attack(
        &mut self,
        category: AttackCategory,
        execution: &AttackExecutionSummary,
    ) -> AgentResult<AttackExecutionSummary>;
}
