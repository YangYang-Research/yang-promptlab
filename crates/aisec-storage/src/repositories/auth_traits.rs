use async_trait::async_trait;
use aisec_core::AisecResult;

use crate::auth_models::*;

#[async_trait]
pub trait AuthProfileRepository: Send + Sync {
    async fn create(&self, input: CreateAuthProfile) -> AisecResult<AuthProfile>;
    async fn get(&self, id: &str) -> AisecResult<AuthProfile>;
    async fn list_by_project(&self, project_id: &str) -> AisecResult<Vec<AuthProfile>>;
    async fn list_all(&self) -> AisecResult<Vec<AuthProfile>>;
    async fn update(&self, id: &str, input: UpdateAuthProfile) -> AisecResult<AuthProfile>;
    async fn update_config_and_reference(
        &self,
        id: &str,
        config_json: &serde_json::Value,
        credential_reference_id: Option<&str>,
    ) -> AisecResult<AuthProfile>;
    async fn delete(&self, id: &str) -> AisecResult<()>;
}

#[async_trait]
pub trait AuthSessionRepository: Send + Sync {
    async fn create(&self, input: CreateAuthSessionRecord) -> AisecResult<AuthSessionRecord>;
    async fn get(&self, id: &str) -> AisecResult<AuthSessionRecord>;
    async fn list_by_profile(&self, profile_id: &str) -> AisecResult<Vec<AuthSessionRecord>>;
    async fn list_all(&self) -> AisecResult<Vec<AuthSessionRecord>>;
    async fn list_legacy_with_plaintext_secrets(&self) -> AisecResult<Vec<AuthSessionRecord>>;
    async fn apply_secure_migration(
        &self,
        id: &str,
        credential_reference_id: &str,
    ) -> AisecResult<AuthSessionRecord>;
    async fn update(&self, id: &str, input: UpdateAuthSessionRecord) -> AisecResult<AuthSessionRecord>;
    async fn delete(&self, id: &str) -> AisecResult<()>;
}

#[async_trait]
pub trait AuthRecordingRepository: Send + Sync {
    async fn create(&self, input: CreateAuthRecordingRecord) -> AisecResult<AuthRecordingRecord>;
    async fn get(&self, id: &str) -> AisecResult<AuthRecordingRecord>;
    async fn list_by_profile(&self, profile_id: &str) -> AisecResult<Vec<AuthRecordingRecord>>;
    async fn delete(&self, id: &str) -> AisecResult<()>;
}
