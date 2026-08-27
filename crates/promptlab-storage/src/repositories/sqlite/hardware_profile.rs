use async_trait::async_trait;
use sqlx::SqlitePool;

use promptlab_core::PromptLabResult;

use crate::error::StorageResultExt;
use crate::models::{HardwareProfileRecord, HardwareProfileRow};
use crate::repositories::HardwareProfileRepository;
use crate::util::now;

pub const HARDWARE_PROFILE_ID: &str = "default";

#[derive(Clone)]
pub struct SqliteHardwareProfileRepository {
    pool: SqlitePool,
}

impl SqliteHardwareProfileRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn to_record(row: HardwareProfileRow) -> HardwareProfileRecord {
    HardwareProfileRecord {
        id: row.id,
        profile_json: row.profile_json,
        updated_at: row.updated_at,
    }
}

#[async_trait]
impl HardwareProfileRepository for SqliteHardwareProfileRepository {
    async fn get(&self) -> PromptLabResult<Option<HardwareProfileRecord>> {
        let row = sqlx::query_as::<_, HardwareProfileRow>(
            r#"
            SELECT id, profile_json, updated_at
            FROM hardware_profile
            WHERE id = ?
            "#,
        )
        .bind(HARDWARE_PROFILE_ID)
        .fetch_optional(&self.pool)
        .await
        .map_storage()?;

        Ok(row.map(to_record))
    }

    async fn upsert(&self, profile_json: &str) -> PromptLabResult<HardwareProfileRecord> {
        let timestamp = now();
        sqlx::query(
            r#"
            INSERT INTO hardware_profile (id, profile_json, updated_at)
            VALUES (?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
              profile_json = excluded.profile_json,
              updated_at = excluded.updated_at
            "#,
        )
        .bind(HARDWARE_PROFILE_ID)
        .bind(profile_json)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_storage()?;

        Ok(HardwareProfileRecord {
            id: HARDWARE_PROFILE_ID.to_string(),
            profile_json: profile_json.to_string(),
            updated_at: timestamp,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::test_utils::test_database;
    use crate::repositories::HardwareProfileRepository;

    #[tokio::test]
    async fn upsert_and_get_roundtrip() {
        let db = test_database().await;
        let repo = SqliteHardwareProfileRepository::new(db.pool().clone());

        assert!(repo.get().await.expect("get").is_none());

        let json = r#"{"os":"macos","cpuCores":8}"#;
        let saved = repo.upsert(json).await.expect("upsert");
        assert_eq!(saved.profile_json, json);

        let loaded = repo.get().await.expect("get").expect("row");
        assert_eq!(loaded.profile_json, json);

        let updated = repo.upsert(r#"{"os":"linux"}"#).await.expect("upsert");
        assert_eq!(updated.profile_json, r#"{"os":"linux"}"#);
        assert_eq!(
            repo.get().await.expect("get").expect("row").profile_json,
            r#"{"os":"linux"}"#
        );
    }
}
