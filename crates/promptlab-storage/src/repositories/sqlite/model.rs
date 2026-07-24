use async_trait::async_trait;
use sqlx::SqlitePool;

use promptlab_core::PromptLabResult;

use crate::error::StorageResultExt;
use crate::models::{CreateModel, ModelRecord, UpdateModel};
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
}

#[async_trait]
impl ModelRepository for SqliteModelRepository {
    async fn create(&self, input: CreateModel) -> PromptLabResult<ModelRecord> {
        let id = new_id();
        let timestamp = now();
        let format = input.format.unwrap_or_else(|| "gguf".to_string());
        let metadata_json = crate::models::json_string(&input.metadata_json)?;

        sqlx::query(
            r#"
            INSERT INTO models (
                id, name, file_path, format, checksum_sha256, size_bytes,
                metadata_json, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&input.name)
        .bind(&input.file_path)
        .bind(&format)
        .bind(&input.checksum_sha256)
        .bind(input.size_bytes)
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
        sqlx::query_as::<_, ModelRecord>("SELECT * FROM models ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await
            .map_storage()
    }

    async fn update(&self, id: &str, input: UpdateModel) -> PromptLabResult<ModelRecord> {
        let existing = self.get(id).await?;
        let name = input.name.unwrap_or(existing.name);
        let file_path = input.file_path.unwrap_or(existing.file_path);
        let format = input.format.unwrap_or(existing.format);
        let checksum_sha256 = input.checksum_sha256.or(existing.checksum_sha256);
        let size_bytes = input.size_bytes.or(existing.size_bytes);
        let metadata_json = match input.metadata_json {
            Some(value) => Some(crate::models::json_string_required(&value)?),
            None => existing.metadata_json,
        };
        let updated_at = now();

        let result = sqlx::query(
            r#"
            UPDATE models
            SET name = ?, file_path = ?, format = ?, checksum_sha256 = ?,
                size_bytes = ?, metadata_json = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&name)
        .bind(&file_path)
        .bind(&format)
        .bind(&checksum_sha256)
        .bind(size_bytes)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::test_utils::test_database;

    #[tokio::test]
    async fn model_registry_crud() {
        let db = test_database().await;
        let repo = db.repositories().models();

        let model = repo
            .create(CreateModel {
                name: "llama-3-8b".into(),
                file_path: "/vault/models/llama.gguf".into(),
                format: None,
                checksum_sha256: Some("abc123".into()),
                size_bytes: Some(4_000_000_000),
                metadata_json: Some(serde_json::json!({"quant": "Q4_K_M"})),
            })
            .await
            .unwrap();

        assert_eq!(model.format, "gguf");
        repo.delete(&model.id).await.unwrap();
    }
}
