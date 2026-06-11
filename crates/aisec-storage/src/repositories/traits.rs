use async_trait::async_trait;
use aisec_core::AisecResult;

use crate::models::*;

#[async_trait]
pub trait ProjectRepository: Send + Sync {
    async fn create(&self, input: CreateProject) -> AisecResult<Project>;
    async fn get(&self, id: &str) -> AisecResult<Project>;
    async fn list(&self) -> AisecResult<Vec<Project>>;
    async fn update(&self, id: &str, input: UpdateProject) -> AisecResult<Project>;
    async fn delete(&self, id: &str) -> AisecResult<()>;
}

#[async_trait]
pub trait TargetRepository: Send + Sync {
    async fn create(&self, input: CreateTarget) -> AisecResult<Target>;
    async fn get(&self, id: &str) -> AisecResult<Target>;
    async fn list_by_project(&self, project_id: &str) -> AisecResult<Vec<Target>>;
    async fn update(&self, id: &str, input: UpdateTarget) -> AisecResult<Target>;
    async fn delete(&self, id: &str) -> AisecResult<()>;
}

#[async_trait]
pub trait ScanRepository: Send + Sync {
    async fn create(&self, input: CreateScan) -> AisecResult<Scan>;
    async fn get(&self, id: &str) -> AisecResult<Scan>;
    async fn list_by_project(&self, project_id: &str) -> AisecResult<Vec<Scan>>;
    async fn update(&self, id: &str, input: UpdateScan) -> AisecResult<Scan>;
    async fn delete(&self, id: &str) -> AisecResult<()>;
}

#[async_trait]
pub trait FindingRepository: Send + Sync {
    async fn create(&self, input: CreateFinding) -> AisecResult<Finding>;
    async fn get(&self, id: &str) -> AisecResult<Finding>;
    async fn list_by_scan(&self, scan_id: &str) -> AisecResult<Vec<Finding>>;
    async fn list_by_project(&self, project_id: &str) -> AisecResult<Vec<Finding>>;
    async fn search(&self, query: &str, limit: i64) -> AisecResult<Vec<Finding>>;
    async fn update(&self, id: &str, input: UpdateFinding) -> AisecResult<Finding>;
    async fn delete(&self, id: &str) -> AisecResult<()>;
}

#[async_trait]
pub trait PayloadRepository: Send + Sync {
    async fn create(&self, input: CreatePayload) -> AisecResult<Payload>;
    async fn get(&self, id: &str) -> AisecResult<Payload>;
    async fn list(&self) -> AisecResult<Vec<Payload>>;
    async fn list_by_project(&self, project_id: &str) -> AisecResult<Vec<Payload>>;
    async fn update(&self, id: &str, input: UpdatePayload) -> AisecResult<Payload>;
    async fn delete(&self, id: &str) -> AisecResult<()>;
}

#[async_trait]
pub trait AttackResultRepository: Send + Sync {
    async fn create(&self, input: CreateAttackResult) -> AisecResult<AttackResult>;
    async fn get(&self, id: &str) -> AisecResult<AttackResult>;
    async fn list_by_scan(&self, scan_id: &str) -> AisecResult<Vec<AttackResult>>;
    async fn update(&self, id: &str, input: UpdateAttackResult) -> AisecResult<AttackResult>;
    async fn delete(&self, id: &str) -> AisecResult<()>;
}

#[async_trait]
pub trait ReportRepository: Send + Sync {
    async fn create(&self, input: CreateReport) -> AisecResult<Report>;
    async fn get(&self, id: &str) -> AisecResult<Report>;
    async fn list_by_project(&self, project_id: &str) -> AisecResult<Vec<Report>>;
    async fn update(&self, id: &str, input: UpdateReport) -> AisecResult<Report>;
    async fn delete(&self, id: &str) -> AisecResult<()>;
}

#[async_trait]
pub trait ModelRepository: Send + Sync {
    async fn create(&self, input: CreateModel) -> AisecResult<ModelRecord>;
    async fn get(&self, id: &str) -> AisecResult<ModelRecord>;
    async fn list(&self) -> AisecResult<Vec<ModelRecord>>;
    async fn update(&self, id: &str, input: UpdateModel) -> AisecResult<ModelRecord>;
    async fn delete(&self, id: &str) -> AisecResult<()>;
}

#[async_trait]
pub trait PluginRepository: Send + Sync {
    async fn create(&self, input: CreatePlugin) -> AisecResult<Plugin>;
    async fn get(&self, id: &str) -> AisecResult<Plugin>;
    async fn get_by_plugin_id(&self, plugin_id: &str) -> AisecResult<Plugin>;
    async fn list(&self) -> AisecResult<Vec<Plugin>>;
    async fn update(&self, id: &str, input: UpdatePlugin) -> AisecResult<Plugin>;
    async fn delete(&self, id: &str) -> AisecResult<()>;
}
