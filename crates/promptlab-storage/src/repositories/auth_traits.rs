use async_trait::async_trait;
use promptlab_core::PromptLabResult;

use crate::auth_models::*;

#[async_trait]
pub trait AuthProfileRepository: Send + Sync {
    async fn create(&self, input: CreateAuthProfile) -> PromptLabResult<AuthProfile>;
    async fn get(&self, id: &str) -> PromptLabResult<AuthProfile>;
    async fn list_by_project(&self, project_id: &str) -> PromptLabResult<Vec<AuthProfile>>;
    async fn list_all(&self) -> PromptLabResult<Vec<AuthProfile>>;
    async fn update(&self, id: &str, input: UpdateAuthProfile) -> PromptLabResult<AuthProfile>;
    async fn update_config_and_reference(
        &self,
        id: &str,
        config_json: &serde_json::Value,
        credential_reference_id: Option<&str>,
    ) -> PromptLabResult<AuthProfile>;
    async fn delete(&self, id: &str) -> PromptLabResult<()>;
}

#[async_trait]
pub trait AuthSessionRepository: Send + Sync {
    async fn create(&self, input: CreateAuthSessionRecord) -> PromptLabResult<AuthSessionRecord>;
    async fn get(&self, id: &str) -> PromptLabResult<AuthSessionRecord>;
    async fn list_by_profile(&self, profile_id: &str) -> PromptLabResult<Vec<AuthSessionRecord>>;
    async fn list_all(&self) -> PromptLabResult<Vec<AuthSessionRecord>>;
    async fn list_legacy_with_plaintext_secrets(&self) -> PromptLabResult<Vec<AuthSessionRecord>>;
    async fn apply_secure_migration(
        &self,
        id: &str,
        credential_reference_id: &str,
    ) -> PromptLabResult<AuthSessionRecord>;
    async fn update(&self, id: &str, input: UpdateAuthSessionRecord) -> PromptLabResult<AuthSessionRecord>;
    async fn delete(&self, id: &str) -> PromptLabResult<()>;
}

#[async_trait]
pub trait AuthRecordingRepository: Send + Sync {
    async fn create(&self, input: CreateAuthRecordingRecord) -> PromptLabResult<AuthRecordingRecord>;
    async fn get(&self, id: &str) -> PromptLabResult<AuthRecordingRecord>;
    async fn list_by_profile(&self, profile_id: &str) -> PromptLabResult<Vec<AuthRecordingRecord>>;
    async fn delete(&self, id: &str) -> PromptLabResult<()>;
}
