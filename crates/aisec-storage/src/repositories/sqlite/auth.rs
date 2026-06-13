use async_trait::async_trait;
use sqlx::SqlitePool;

use aisec_core::AisecResult;

use crate::auth_models::*;
use crate::error::StorageResultExt;
use crate::repositories::{
    AuthProfileRepository, AuthRecordingRepository, AuthSessionRepository,
};
use crate::util::{ensure_rows_affected, new_id, now};

#[derive(Clone)]
pub struct SqliteAuthProfileRepository {
    pool: SqlitePool,
}

impl SqliteAuthProfileRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuthProfileRepository for SqliteAuthProfileRepository {
    async fn create(&self, input: CreateAuthProfile) -> AisecResult<AuthProfile> {
        let id = new_id();
        let ts = now();
        let config = json_required(&input.config_json)?;

        sqlx::query(
            "INSERT INTO auth_profiles (id, project_id, name, method, config_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.project_id)
        .bind(&input.name)
        .bind(&input.method)
        .bind(&config)
        .bind(ts)
        .bind(ts)
        .execute(&self.pool)
        .await
        .map_storage()?;

        self.get(&id).await
    }

    async fn get(&self, id: &str) -> AisecResult<AuthProfile> {
        sqlx::query_as::<_, AuthProfile>("SELECT * FROM auth_profiles WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_storage()
    }

    async fn list_by_project(&self, project_id: &str) -> AisecResult<Vec<AuthProfile>> {
        sqlx::query_as::<_, AuthProfile>(
            "SELECT * FROM auth_profiles WHERE project_id = ? ORDER BY created_at DESC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_storage()
    }

    async fn update(&self, id: &str, input: UpdateAuthProfile) -> AisecResult<AuthProfile> {
        let existing = self.get(id).await?;
        let name = input.name.unwrap_or(existing.name);
        let config = match input.config_json {
            Some(v) => json_required(&v)?,
            None => existing.config_json,
        };
        let ts = now();

        let result = sqlx::query(
            "UPDATE auth_profiles SET name = ?, config_json = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&name)
        .bind(&config)
        .bind(ts)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_storage()?;

        ensure_rows_affected(result, "auth_profile")?;
        self.get(id).await
    }

    async fn delete(&self, id: &str) -> AisecResult<()> {
        let result = sqlx::query("DELETE FROM auth_profiles WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_storage()?;
        ensure_rows_affected(result, "auth_profile")
    }
}

#[derive(Clone)]
pub struct SqliteAuthSessionRepository {
    pool: SqlitePool,
}

impl SqliteAuthSessionRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuthSessionRepository for SqliteAuthSessionRepository {
    async fn create(&self, input: CreateAuthSessionRecord) -> AisecResult<AuthSessionRecord> {
        let id = new_id();
        let ts = now();
        let status = input.status.unwrap_or_else(|| "active".to_string());
        let cookies = json_opt(&input.cookies_json)?;
        let tokens = json_opt(&input.tokens_json)?;

        let validation_status = input
            .validation_status
            .unwrap_or_else(|| "valid".to_string());

        sqlx::query(
            "INSERT INTO auth_sessions (id, profile_id, status, cookies_json, tokens_json,
             storage_state_path, expires_at, validation_status, user_identity, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.profile_id)
        .bind(&status)
        .bind(&cookies)
        .bind(&tokens)
        .bind(&input.storage_state_path)
        .bind(input.expires_at)
        .bind(&validation_status)
        .bind(&input.user_identity)
        .bind(ts)
        .bind(ts)
        .execute(&self.pool)
        .await
        .map_storage()?;

        self.get(&id).await
    }

    async fn get(&self, id: &str) -> AisecResult<AuthSessionRecord> {
        sqlx::query_as::<_, AuthSessionRecord>("SELECT * FROM auth_sessions WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_storage()
    }

    async fn list_by_profile(&self, profile_id: &str) -> AisecResult<Vec<AuthSessionRecord>> {
        sqlx::query_as::<_, AuthSessionRecord>(
            "SELECT * FROM auth_sessions WHERE profile_id = ? ORDER BY created_at DESC",
        )
        .bind(profile_id)
        .fetch_all(&self.pool)
        .await
        .map_storage()
    }

    async fn update(&self, id: &str, input: UpdateAuthSessionRecord) -> AisecResult<AuthSessionRecord> {
        let existing = self.get(id).await?;
        let status = input.status.unwrap_or(existing.status);
        let cookies = match input.cookies_json {
            Some(v) => Some(json_required(&v)?),
            None => existing.cookies_json,
        };
        let tokens = match input.tokens_json {
            Some(v) => Some(json_required(&v)?),
            None => existing.tokens_json,
        };
        let storage_state_path = input.storage_state_path.or(existing.storage_state_path);
        let expires_at = input.expires_at.or(existing.expires_at);
        let validation_status = input
            .validation_status
            .unwrap_or(existing.validation_status);
        let last_validated_at = input
            .last_validated_at
            .or(existing.last_validated_at);
        let user_identity = input.user_identity.or(existing.user_identity);
        let ts = now();

        let result = sqlx::query(
            "UPDATE auth_sessions SET status = ?, cookies_json = ?, tokens_json = ?,
             storage_state_path = ?, expires_at = ?, validation_status = ?,
             last_validated_at = ?, user_identity = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&status)
        .bind(&cookies)
        .bind(&tokens)
        .bind(&storage_state_path)
        .bind(expires_at)
        .bind(&validation_status)
        .bind(last_validated_at)
        .bind(&user_identity)
        .bind(ts)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_storage()?;

        ensure_rows_affected(result, "auth_session")?;
        self.get(id).await
    }

    async fn delete(&self, id: &str) -> AisecResult<()> {
        let result = sqlx::query("DELETE FROM auth_sessions WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_storage()?;
        ensure_rows_affected(result, "auth_session")
    }
}

#[derive(Clone)]
pub struct SqliteAuthRecordingRepository {
    pool: SqlitePool,
}

impl SqliteAuthRecordingRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuthRecordingRepository for SqliteAuthRecordingRepository {
    async fn create(&self, input: CreateAuthRecordingRecord) -> AisecResult<AuthRecordingRecord> {
        let id = new_id();
        let ts = now();
        let steps = json_required(&input.steps_json)?;
        let metadata = json_opt(&input.metadata_json)?;

        sqlx::query(
            "INSERT INTO auth_recordings (id, profile_id, steps_json, storage_state_path, metadata_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.profile_id)
        .bind(&steps)
        .bind(&input.storage_state_path)
        .bind(&metadata)
        .bind(ts)
        .execute(&self.pool)
        .await
        .map_storage()?;

        self.get(&id).await
    }

    async fn get(&self, id: &str) -> AisecResult<AuthRecordingRecord> {
        sqlx::query_as::<_, AuthRecordingRecord>("SELECT * FROM auth_recordings WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_storage()
    }

    async fn list_by_profile(&self, profile_id: &str) -> AisecResult<Vec<AuthRecordingRecord>> {
        sqlx::query_as::<_, AuthRecordingRecord>(
            "SELECT * FROM auth_recordings WHERE profile_id = ? ORDER BY created_at DESC",
        )
        .bind(profile_id)
        .fetch_all(&self.pool)
        .await
        .map_storage()
    }

    async fn delete(&self, id: &str) -> AisecResult<()> {
        let result = sqlx::query("DELETE FROM auth_recordings WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_storage()?;
        ensure_rows_affected(result, "auth_recording")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::test_utils::test_database;

    #[tokio::test]
    async fn auth_profile_session_recording_crud() {
        let db = test_database().await;
        let pool = db.pool().clone();
        let profiles = SqliteAuthProfileRepository::new(pool.clone());
        let sessions = SqliteAuthSessionRepository::new(pool.clone());
        let recordings = SqliteAuthRecordingRepository::new(pool);

        let profile = profiles
            .create(CreateAuthProfile {
                project_id: None,
                name: "App Login".into(),
                method: "username_password".into(),
                config_json: serde_json::json!({"login_url": "https://app.example.com/login"}),
            })
            .await
            .unwrap();

        let session = sessions
            .create(CreateAuthSessionRecord {
                profile_id: profile.id.clone(),
                status: None,
                cookies_json: Some(serde_json::json!([{"name":"sid","value":"abc"}])),
                tokens_json: None,
                storage_state_path: Some("/vault/session.json".into()),
                expires_at: None,
            })
            .await
            .unwrap();

        assert_eq!(session.profile_id, profile.id);

        let recording = recordings
            .create(CreateAuthRecordingRecord {
                profile_id: profile.id.clone(),
                steps_json: serde_json::json!([{"action":"fill","selector":"#user"}]),
                storage_state_path: None,
                metadata_json: None,
            })
            .await
            .unwrap();

        assert!(!recording.steps_json.is_empty());
    }
}
