use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct AuthProfile {
    pub id: String,
    pub project_id: Option<String>,
    pub name: String,
    pub method: String,
    pub config_json: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct AuthSessionRecord {
    pub id: String,
    pub profile_id: String,
    pub status: String,
    pub cookies_json: Option<String>,
    pub tokens_json: Option<String>,
    pub storage_state_path: Option<String>,
    pub expires_at: Option<OffsetDateTime>,
    pub validation_status: String,
    pub last_validated_at: Option<OffsetDateTime>,
    pub user_identity: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct AuthRecordingRecord {
    pub id: String,
    pub profile_id: String,
    pub steps_json: String,
    pub storage_state_path: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAuthProfile {
    pub project_id: Option<String>,
    pub name: String,
    pub method: String,
    pub config_json: serde_json::Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateAuthProfile {
    pub name: Option<String>,
    pub config_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAuthSessionRecord {
    pub profile_id: String,
    pub status: Option<String>,
    pub cookies_json: Option<serde_json::Value>,
    pub tokens_json: Option<serde_json::Value>,
    pub storage_state_path: Option<String>,
    pub expires_at: Option<OffsetDateTime>,
    pub validation_status: Option<String>,
    pub user_identity: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateAuthSessionRecord {
    pub status: Option<String>,
    pub cookies_json: Option<serde_json::Value>,
    pub tokens_json: Option<serde_json::Value>,
    pub storage_state_path: Option<String>,
    pub expires_at: Option<OffsetDateTime>,
    pub validation_status: Option<String>,
    pub last_validated_at: Option<OffsetDateTime>,
    pub user_identity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAuthRecordingRecord {
    pub profile_id: String,
    pub steps_json: serde_json::Value,
    pub storage_state_path: Option<String>,
    pub metadata_json: Option<serde_json::Value>,
}

pub(crate) fn json_opt(value: &Option<serde_json::Value>) -> aisec_core::AisecResult<Option<String>> {
    match value {
        Some(v) => Ok(Some(serde_json::to_string(v).map_err(|e| {
            aisec_core::AisecError::invalid_input(e.to_string())
        })?)),
        None => Ok(None),
    }
}

pub(crate) fn json_required(value: &serde_json::Value) -> aisec_core::AisecResult<String> {
    serde_json::to_string(value).map_err(|e| aisec_core::AisecError::invalid_input(e.to_string()))
}
