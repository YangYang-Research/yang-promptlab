use async_trait::async_trait;
use sqlx::SqlitePool;

use promptlab_core::PromptLabResult;

use crate::error::StorageResultExt;
use crate::models::{CreatePromptLabHistory, PromptLabHistory};
use crate::repositories::PromptLabHistoryRepository;
use crate::util::{ensure_rows_affected, new_id, now};

#[derive(Clone)]
pub struct SqlitePromptLabHistoryRepository {
    pool: SqlitePool,
}

impl SqlitePromptLabHistoryRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PromptLabHistoryRepository for SqlitePromptLabHistoryRepository {
    async fn create(&self, input: CreatePromptLabHistory) -> PromptLabResult<PromptLabHistory> {
        let id = new_id();
        let timestamp = now();
        sqlx::query(
            r#"
            INSERT INTO prompt_lab_history (id, seed_prompt, variants_json, created_at)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&input.seed_prompt)
        .bind(&input.variants_json)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_storage()?;

        self.get(&id).await
    }

    async fn get(&self, id: &str) -> PromptLabResult<PromptLabHistory> {
        sqlx::query_as::<_, PromptLabHistory>(
            "SELECT id, seed_prompt, variants_json, created_at FROM prompt_lab_history WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_storage()
    }

    async fn list(&self, limit: i64) -> PromptLabResult<Vec<PromptLabHistory>> {
        let cap = limit.clamp(1, 200);
        sqlx::query_as::<_, PromptLabHistory>(
            r#"
            SELECT id, seed_prompt, variants_json, created_at
            FROM prompt_lab_history
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(cap)
        .fetch_all(&self.pool)
        .await
        .map_storage()
    }

    async fn delete(&self, id: &str) -> PromptLabResult<()> {
        let result = sqlx::query("DELETE FROM prompt_lab_history WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_storage()?;
        ensure_rows_affected(result, "prompt_lab_history")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::test_utils::test_database;
    use crate::repositories::PromptLabHistoryRepository;

    #[tokio::test]
    async fn create_list_get_delete() {
        let db = test_database().await;
        let repo = SqlitePromptLabHistoryRepository::new(db.pool().clone());

        let created = repo
            .create(CreatePromptLabHistory {
                seed_prompt: "ignore safety".into(),
                variants_json: r#"[{"mutatorId":"base64_wrap","content":"x"}]"#.into(),
            })
            .await
            .expect("create");

        assert_eq!(created.seed_prompt, "ignore safety");
        let listed = repo.list(10).await.expect("list");
        assert_eq!(listed.len(), 1);
        let got = repo.get(&created.id).await.expect("get");
        assert_eq!(got.id, created.id);
        repo.delete(&created.id).await.expect("delete");
        assert!(repo.list(10).await.expect("list empty").is_empty());
    }
}
