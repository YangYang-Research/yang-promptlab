use async_trait::async_trait;
use promptlab_core::PromptLabResult;

use crate::models::*;

#[async_trait]
pub trait ProjectRepository: Send + Sync {
    async fn create(&self, input: CreateProject) -> PromptLabResult<Project>;
    async fn get(&self, id: &str) -> PromptLabResult<Project>;
    async fn list(&self) -> PromptLabResult<Vec<Project>>;
    async fn update(&self, id: &str, input: UpdateProject) -> PromptLabResult<Project>;
    async fn delete(&self, id: &str) -> PromptLabResult<()>;
}

#[async_trait]
pub trait TargetRepository: Send + Sync {
    async fn create(&self, input: CreateTarget) -> PromptLabResult<Target>;
    async fn get(&self, id: &str) -> PromptLabResult<Target>;
    async fn list_by_project(&self, project_id: &str) -> PromptLabResult<Vec<Target>>;
    async fn list_all(&self) -> PromptLabResult<Vec<Target>>;
    async fn update(&self, id: &str, input: UpdateTarget) -> PromptLabResult<Target>;
    async fn update_descriptor(&self, id: &str, descriptor_json: &str) -> PromptLabResult<Target>;
    async fn update_profile(&self, id: &str, profile_json: &str) -> PromptLabResult<Target>;
    async fn delete(&self, id: &str) -> PromptLabResult<()>;
}

#[async_trait]
pub trait ScanRepository: Send + Sync {
    async fn create(&self, input: CreateScan) -> PromptLabResult<Scan>;
    async fn get(&self, id: &str) -> PromptLabResult<Scan>;
    async fn list_by_project(&self, project_id: &str) -> PromptLabResult<Vec<Scan>>;
    /// Scans left in a non-terminal in-memory state (e.g. after an abrupt app exit).
    async fn list_interrupted(&self) -> PromptLabResult<Vec<Scan>>;
    async fn update(&self, id: &str, input: UpdateScan) -> PromptLabResult<Scan>;
    async fn delete(&self, id: &str) -> PromptLabResult<()>;
}

#[async_trait]
pub trait EndpointRepository: Send + Sync {
    async fn create(&self, input: CreateEndpoint) -> PromptLabResult<Endpoint>;
    async fn create_many(&self, inputs: Vec<CreateEndpoint>) -> PromptLabResult<Vec<Endpoint>>;
    async fn get(&self, id: &str) -> PromptLabResult<Endpoint>;
    async fn list_by_scan(&self, scan_id: &str) -> PromptLabResult<Vec<Endpoint>>;
    async fn update(&self, id: &str, input: UpdateEndpoint) -> PromptLabResult<Endpoint>;
    async fn delete_by_scan(&self, scan_id: &str) -> PromptLabResult<u64>;
}

#[async_trait]
pub trait FindingRepository: Send + Sync {
    async fn create(&self, input: CreateFinding) -> PromptLabResult<Finding>;
    async fn get(&self, id: &str) -> PromptLabResult<Finding>;
    async fn list_by_scan(&self, scan_id: &str) -> PromptLabResult<Vec<Finding>>;
    async fn list_by_project(&self, project_id: &str) -> PromptLabResult<Vec<Finding>>;
    async fn search(&self, query: &str, limit: i64) -> PromptLabResult<Vec<Finding>>;
    async fn update(&self, id: &str, input: UpdateFinding) -> PromptLabResult<Finding>;
    async fn delete(&self, id: &str) -> PromptLabResult<()>;
}

#[async_trait]
pub trait PayloadRepository: Send + Sync {
    async fn create(&self, input: CreatePayload) -> PromptLabResult<Payload>;
    async fn get(&self, id: &str) -> PromptLabResult<Payload>;
    async fn list(&self) -> PromptLabResult<Vec<Payload>>;
    async fn list_by_project(&self, project_id: &str) -> PromptLabResult<Vec<Payload>>;
    async fn update(&self, id: &str, input: UpdatePayload) -> PromptLabResult<Payload>;
    async fn delete(&self, id: &str) -> PromptLabResult<()>;
}

#[async_trait]
pub trait AttackResultRepository: Send + Sync {
    async fn create(&self, input: CreateAttackResult) -> PromptLabResult<AttackResult>;
    async fn get(&self, id: &str) -> PromptLabResult<AttackResult>;
    async fn list_by_scan(&self, scan_id: &str) -> PromptLabResult<Vec<AttackResult>>;
    async fn update(&self, id: &str, input: UpdateAttackResult) -> PromptLabResult<AttackResult>;
    async fn delete(&self, id: &str) -> PromptLabResult<()>;
}

#[async_trait]
pub trait ReportRepository: Send + Sync {
    async fn create(&self, input: CreateReport) -> PromptLabResult<Report>;
    async fn get(&self, id: &str) -> PromptLabResult<Report>;
    async fn list_by_project(&self, project_id: &str) -> PromptLabResult<Vec<Report>>;
    async fn update(&self, id: &str, input: UpdateReport) -> PromptLabResult<Report>;
    async fn delete(&self, id: &str) -> PromptLabResult<()>;
}

#[async_trait]
pub trait ModelRepository: Send + Sync {
    async fn create(&self, input: CreateModel) -> PromptLabResult<ModelRecord>;
    async fn get(&self, id: &str) -> PromptLabResult<ModelRecord>;
    async fn list(&self) -> PromptLabResult<Vec<ModelRecord>>;
    async fn update(&self, id: &str, input: UpdateModel) -> PromptLabResult<ModelRecord>;
    async fn delete(&self, id: &str) -> PromptLabResult<()>;
}

#[async_trait]
pub trait PluginRepository: Send + Sync {
    async fn create(&self, input: CreatePlugin) -> PromptLabResult<Plugin>;
    async fn get(&self, id: &str) -> PromptLabResult<Plugin>;
    async fn get_by_plugin_id(&self, plugin_id: &str) -> PromptLabResult<Plugin>;
    async fn list(&self) -> PromptLabResult<Vec<Plugin>>;
    async fn update(&self, id: &str, input: UpdatePlugin) -> PromptLabResult<Plugin>;
    async fn delete(&self, id: &str) -> PromptLabResult<()>;
}

#[async_trait]
pub trait AttackCatalogRepository: Send + Sync {
    async fn get(&self, id: &str) -> PromptLabResult<AttackCatalogTechnique>;
    async fn list(&self) -> PromptLabResult<Vec<AttackCatalogTechnique>>;
    async fn list_enabled(&self) -> PromptLabResult<Vec<AttackCatalogTechnique>>;
    async fn list_by_category(&self, category_id: &str) -> PromptLabResult<Vec<AttackCatalogTechnique>>;
    /// Insert missing seed rows; refresh factory defaults without clobbering user edits.
    async fn seed_from(&self, entries: Vec<UpsertAttackCatalogTechnique>) -> PromptLabResult<u64>;
    async fn update(
        &self,
        id: &str,
        input: UpdateAttackCatalogTechnique,
    ) -> PromptLabResult<AttackCatalogTechnique>;
    async fn reset_content(&self, id: &str) -> PromptLabResult<AttackCatalogTechnique>;
}

#[async_trait]
pub trait RuntimeTrafficRepository: Send + Sync {
    async fn insert_many(&self, events: Vec<CreateRuntimeTrafficEvent>) -> PromptLabResult<u64>;
    async fn list_between(&self, start_ms: i64, end_ms: i64) -> PromptLabResult<Vec<RuntimeTrafficEvent>>;
    async fn counters(&self) -> PromptLabResult<RuntimeTrafficCounters>;
    async fn prune_before(&self, cutoff_ms: i64) -> PromptLabResult<u64>;
}

#[async_trait]
pub trait JudgeRoleWeightsRepository: Send + Sync {
    async fn get(&self) -> PromptLabResult<JudgeRoleWeights>;
    async fn update(&self, input: UpdateJudgeRoleWeights) -> PromptLabResult<JudgeRoleWeights>;
}

#[async_trait]
pub trait MutatorSettingsRepository: Send + Sync {
    async fn get(&self) -> PromptLabResult<MutatorSettings>;
    async fn update(&self, input: UpdateMutatorSettings) -> PromptLabResult<MutatorSettings>;
}

#[async_trait]
pub trait AgentShortTermMemoryRepository: Send + Sync {
    async fn create(&self, input: CreateAgentShortTermMemory) -> PromptLabResult<AgentShortTermMemory>;
    async fn get(&self, id: &str) -> PromptLabResult<AgentShortTermMemory>;
    async fn list_by_session(&self, session_id: &str) -> PromptLabResult<Vec<AgentShortTermMemory>>;
    async fn list_by_session_agent(
        &self,
        session_id: &str,
        agent_id: &str,
    ) -> PromptLabResult<Vec<AgentShortTermMemory>>;
    /// List STM sessions (AgentCore ListSessions), newest activity first.
    ///
    /// `prefix` filters `session_id` (e.g. `yazg-chat:`). Expired rows are excluded.
    async fn list_sessions(
        &self,
        prefix: Option<&str>,
        limit: usize,
    ) -> PromptLabResult<Vec<crate::models::AgentStmSessionSummary>>;
    async fn delete(&self, id: &str) -> PromptLabResult<()>;
    async fn delete_by_session(&self, session_id: &str) -> PromptLabResult<u64>;
    /// Remove rows with `expires_at` at or before `cutoff`.
    async fn prune_expired(&self, cutoff: time::OffsetDateTime) -> PromptLabResult<u64>;
}

#[async_trait]
pub trait AgentLongTermMemoryRepository: Send + Sync {
    async fn upsert(&self, input: UpsertAgentLongTermMemory) -> PromptLabResult<AgentLongTermMemory>;
    async fn get(&self, id: &str) -> PromptLabResult<AgentLongTermMemory>;
    async fn get_by_key(
        &self,
        agent_id: &str,
        scope_type: &str,
        scope_id: &str,
        memory_key: &str,
    ) -> PromptLabResult<AgentLongTermMemory>;
    async fn list_by_scope(
        &self,
        scope_type: &str,
        scope_id: &str,
    ) -> PromptLabResult<Vec<AgentLongTermMemory>>;
    async fn list_by_agent_scope(
        &self,
        agent_id: &str,
        scope_type: &str,
        scope_id: &str,
    ) -> PromptLabResult<Vec<AgentLongTermMemory>>;
    async fn update(&self, id: &str, input: UpdateAgentLongTermMemory) -> PromptLabResult<AgentLongTermMemory>;
    /// Bump access_count + last_accessed_at for retrieval feedback.
    async fn touch(&self, id: &str) -> PromptLabResult<AgentLongTermMemory>;
    async fn delete(&self, id: &str) -> PromptLabResult<()>;
    async fn delete_by_scope(&self, scope_type: &str, scope_id: &str) -> PromptLabResult<u64>;
}
