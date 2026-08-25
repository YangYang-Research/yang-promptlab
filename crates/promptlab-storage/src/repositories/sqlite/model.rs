use async_trait::async_trait;
use sqlx::SqlitePool;

use promptlab_core::PromptLabResult;

use crate::error::StorageResultExt;
use crate::models::{CreateModel, ModelRecord, UpdateModel, UpsertModelEntry};
use crate::repositories::ModelRepository;
use crate::util::{ensure_rows_affected, new_id, now};

#[derive(Clone)]
pub struct SqliteModelRepository {
    pool: SqlitePool,
}

impl SqliteModelRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    async fn insert_upsert(
        &self,
        executor: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
        input: &UpsertModelEntry,
    ) -> PromptLabResult<()> {
        sqlx::query(
            r#"
            INSERT INTO models (
                id, name, provider, format, file_path, checksum_sha256, size_bytes,
                verified, entry_json, metadata_json, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                provider = excluded.provider,
                format = excluded.format,
                file_path = excluded.file_path,
                checksum_sha256 = excluded.checksum_sha256,
                size_bytes = excluded.size_bytes,
                verified = excluded.verified,
                entry_json = excluded.entry_json,
                metadata_json = excluded.metadata_json,
                created_at = excluded.created_at,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&input.id)
        .bind(&input.name)
        .bind(&input.provider)
        .bind(&input.format)
        .bind(&input.file_path)
        .bind(&input.checksum_sha256)
        .bind(input.size_bytes)
        .bind(input.verified)
        .bind(&input.entry_json)
        .bind(&input.metadata_json)
        .bind(input.created_at)
        .bind(input.updated_at)
        .execute(executor)
        .await
        .map_storage()?;
        Ok(())
    }
}

#[async_trait]
impl ModelRepository for SqliteModelRepository {
    async fn create(&self, input: CreateModel) -> PromptLabResult<ModelRecord> {
        let id = new_id();
        let timestamp = now();
        let format = input.format.unwrap_or_else(|| "api".to_string());
        let provider = input.provider.unwrap_or_default();
        let verified = input.verified.unwrap_or(false);
        let entry_json = input.entry_json.unwrap_or_else(|| "{}".to_string());
        let metadata_json = crate::models::json_string(&input.metadata_json)?;

        sqlx::query(
            r#"
            INSERT INTO models (
                id, name, provider, format, file_path, checksum_sha256, size_bytes,
                verified, entry_json, metadata_json, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&input.name)
        .bind(&provider)
        .bind(&format)
        .bind(&input.file_path)
        .bind(&input.checksum_sha256)
        .bind(input.size_bytes)
        .bind(verified)
        .bind(&entry_json)
        .bind(&metadata_json)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_storage()?;

        self.get(&id).await
    }

    async fn get(&self, id: &str) -> PromptLabResult<ModelRecord> {
        sqlx::query_as::<_, ModelRecord>("SELECT * FROM models WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_storage()
    }

    async fn list(&self) -> PromptLabResult<Vec<ModelRecord>> {
        sqlx::query_as::<_, ModelRecord>("SELECT * FROM models ORDER BY updated_at DESC")
            .fetch_all(&self.pool)
            .await
            .map_storage()
    }

    async fn update(&self, id: &str, input: UpdateModel) -> PromptLabResult<ModelRecord> {
        let existing = self.get(id).await?;
        let name = input.name.unwrap_or(existing.name);
        let file_path = input.file_path.unwrap_or(existing.file_path);
        let format = input.format.unwrap_or(existing.format);
        let provider = input.provider.unwrap_or(existing.provider);
        let checksum_sha256 = input.checksum_sha256.or(existing.checksum_sha256);
        let size_bytes = input.size_bytes.or(existing.size_bytes);
        let verified = input.verified.unwrap_or(existing.verified);
        let entry_json = input.entry_json.unwrap_or(existing.entry_json);
        let metadata_json = match input.metadata_json {
            Some(value) => Some(crate::models::json_string_required(&value)?),
            None => existing.metadata_json,
        };
        let updated_at = now();

        let result = sqlx::query(
            r#"
            UPDATE models
            SET name = ?, provider = ?, format = ?, file_path = ?, checksum_sha256 = ?,
                size_bytes = ?, verified = ?, entry_json = ?, metadata_json = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&name)
        .bind(&provider)
        .bind(&format)
        .bind(&file_path)
        .bind(&checksum_sha256)
        .bind(size_bytes)
        .bind(verified)
        .bind(&entry_json)
        .bind(&metadata_json)
        .bind(updated_at)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_storage()?;

        ensure_rows_affected(result, "model")?;
        self.get(id).await
    }

    async fn delete(&self, id: &str) -> PromptLabResult<()> {
        let result = sqlx::query("DELETE FROM models WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_storage()?;

        ensure_rows_affected(result, "model")
    }

    async fn count(&self) -> PromptLabResult<i64> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM models")
            .fetch_one(&self.pool)
            .await
            .map_storage()?;
        Ok(count)
    }

    async fn upsert_entry(&self, input: UpsertModelEntry) -> PromptLabResult<ModelRecord> {
        self.insert_upsert(&self.pool, &input).await?;
        self.get(&input.id).await
    }

    async fn replace_all(&self, entries: Vec<UpsertModelEntry>) -> PromptLabResult<()> {
        let mut tx = self.pool.begin().await.map_storage()?;
        sqlx::query("DELETE FROM models")
            .execute(&mut *tx)
            .await
            .map_storage()?;
        for entry in &entries {
            self.insert_upsert(&mut *tx, entry).await?;
        }
        tx.commit().await.map_storage()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::test_utils::test_database;
    use crate::repositories::ModelRepository;
    use time::OffsetDateTime;

    #[tokio::test]
    async fn model_registry_crud() {
        let db = test_database().await;
        let repo = db.repositories().models();

        let model = repo
            .create(CreateModel {
                name: "llama-3-8b".into(),
                file_path: "/vault/models/llama.gguf".into(),
                format: Some("gguf".into()),
                provider: Some("gguf".into()),
                checksum_sha256: Some("abc123".into()),
                size_bytes: Some(4_000_000_000),
                verified: Some(false),
                entry_json: Some(r#"{"id":"x"}"#.into()),
                metadata_json: Some(serde_json::json!({"quant": "Q4_K_M"})),
            })
            .await
            .unwrap();

        assert_eq!(model.format, "gguf");
        assert_eq!(model.provider, "gguf");
        assert!(!model.verified);
        assert_eq!(repo.count().await.unwrap(), 1);
        repo.delete(&model.id).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn replace_all_and_upsert() {
        let db = test_database().await;
        let repo = db.repositories().models();
        let now = OffsetDateTime::now_utc();

        repo.replace_all(vec![
            UpsertModelEntry {
                id: "a".into(),
                name: "A".into(),
                provider: "remote".into(),
                format: "api".into(),
                file_path: "remote://a".into(),
                checksum_sha256: None,
                size_bytes: None,
                verified: true,
                entry_json: r#"{"id":"a"}"#.into(),
                metadata_json: None,
                created_at: now,
                updated_at: now,
            },
            UpsertModelEntry {
                id: "b".into(),
                name: "B".into(),
                provider: "remote".into(),
                format: "api".into(),
                file_path: "remote://b".into(),
                checksum_sha256: None,
                size_bytes: None,
                verified: false,
                entry_json: r#"{"id":"b"}"#.into(),
                metadata_json: None,
                created_at: now,
                updated_at: now,
            },
        ])
        .await
        .unwrap();
        assert_eq!(repo.count().await.unwrap(), 2);

        repo.upsert_entry(UpsertModelEntry {
            id: "a".into(),
            name: "A2".into(),
            provider: "remote".into(),
            format: "api".into(),
            file_path: "remote://a".into(),
            checksum_sha256: None,
            size_bytes: None,
            verified: true,
            entry_json: r#"{"id":"a","name":"A2"}"#.into(),
            metadata_json: None,
            created_at: now,
            updated_at: now,
        })
        .await
        .unwrap();

        let a = repo.get("a").await.unwrap();
        assert_eq!(a.name, "A2");
        assert_eq!(repo.count().await.unwrap(), 2);

        repo.replace_all(vec![]).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 0);
    }
}
