use async_trait::async_trait;
use sqlx::SqlitePool;

use promptlab_core::PromptLabResult;

use crate::error::StorageResultExt;
use crate::models::{AppSettingRecord, AppSettingRow};
use crate::repositories::AppSettingsRepository;
use crate::util::now;

/// Key for [`promptlab_core::EnvironmentConfig`] JSON.
pub const SETTING_ENVIRONMENT: &str = "environment";
/// Key for AI runtime / inference configuration JSON.
pub const SETTING_AI_RUNTIME_CONFIG: &str = "ai_runtime_config";

#[derive(Clone)]
pub struct SqliteAppSettingsRepository {
    pool: SqlitePool,
}

impl SqliteAppSettingsRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn to_record(row: AppSettingRow) -> AppSettingRecord {
    AppSettingRecord {
        key: row.key,
        value_json: row.value_json,
        updated_at: row.updated_at,
    }
}

#[async_trait]
impl AppSettingsRepository for SqliteAppSettingsRepository {
    async fn get(&self, key: &str) -> PromptLabResult<Option<AppSettingRecord>> {
        let row = sqlx::query_as::<_, AppSettingRow>(
            r#"
            SELECT key, value_json, updated_at
            FROM app_settings
            WHERE key = ?
            "#,
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_storage()?;

        Ok(row.map(to_record))
    }

    async fn upsert(&self, key: &str, value_json: &str) -> PromptLabResult<AppSettingRecord> {
        let timestamp = now();
        sqlx::query(
            r#"
            INSERT INTO app_settings (key, value_json, updated_at)
            VALUES (?, ?, ?)
            ON CONFLICT(key) DO UPDATE SET
              value_json = excluded.value_json,
              updated_at = excluded.updated_at
            "#,
        )
        .bind(key)
        .bind(value_json)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_storage()?;

        Ok(AppSettingRecord {
            key: key.to_string(),
            value_json: value_json.to_string(),
            updated_at: timestamp,
        })
    }

    async fn delete(&self, key: &str) -> PromptLabResult<()> {
        sqlx::query("DELETE FROM app_settings WHERE key = ?")
            .bind(key)
            .execute(&self.pool)
            .await
            .map_storage()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::test_utils::test_database;
    use crate::repositories::AppSettingsRepository;

    #[tokio::test]
    async fn upsert_get_delete_roundtrip() {
        let db = test_database().await;
        let repo = SqliteAppSettingsRepository::new(db.pool().clone());

        assert!(repo.get("environment").await.expect("get").is_none());

        repo.upsert("environment", r#"{"root":"/tmp"}"#)
            .await
            .expect("upsert");
        let loaded = repo.get("environment").await.expect("get").expect("row");
        assert_eq!(loaded.value_json, r#"{"root":"/tmp"}"#);

        repo.delete("environment").await.expect("delete");
        assert!(repo.get("environment").await.expect("get").is_none());
    }
}
