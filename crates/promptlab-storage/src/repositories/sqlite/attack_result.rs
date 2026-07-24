use async_trait::async_trait;
use sqlx::SqlitePool;

use promptlab_core::PromptLabResult;

use crate::error::StorageResultExt;
use crate::models::{CreateAttackResult, AttackResult, UpdateAttackResult};
use crate::repositories::AttackResultRepository;
use crate::util::{ensure_rows_affected, new_id, now};

#[derive(Clone)]
pub struct SqliteAttackResultRepository {
    pool: SqlitePool,
}

impl SqliteAttackResultRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AttackResultRepository for SqliteAttackResultRepository {
    async fn create(&self, input: CreateAttackResult) -> PromptLabResult<AttackResult> {
        let id = new_id();
        let timestamp = now();
        let response_json = crate::models::json_string(&input.response_json)?;
        let evaluated_json = crate::models::json_string(&input.evaluated_json)?;

        sqlx::query(
            r#"
            INSERT INTO attack_results (
                id, scan_id, payload_id, target_id, probe_id, success,
                response_json, evaluated_json, duration_ms, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&input.scan_id)
        .bind(&input.payload_id)
        .bind(&input.target_id)
        .bind(&input.probe_id)
        .bind(input.success)
        .bind(&response_json)
        .bind(&evaluated_json)
        .bind(input.duration_ms)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_storage()?;

        self.get(&id).await
    }

    async fn get(&self, id: &str) -> PromptLabResult<AttackResult> {
        sqlx::query_as::<_, AttackResult>("SELECT * FROM attack_results WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_storage()
    }

    async fn list_by_scan(&self, scan_id: &str) -> PromptLabResult<Vec<AttackResult>> {
        sqlx::query_as::<_, AttackResult>(
            "SELECT * FROM attack_results WHERE scan_id = ? ORDER BY created_at DESC",
        )
        .bind(scan_id)
        .fetch_all(&self.pool)
        .await
        .map_storage()
    }

    async fn update(&self, id: &str, input: UpdateAttackResult) -> PromptLabResult<AttackResult> {
        let existing = self.get(id).await?;
        let success = input.success.unwrap_or(existing.success);
        let response_json = match input.response_json {
            Some(value) => Some(crate::models::json_string_required(&value)?),
            None => existing.response_json,
        };
        let evaluated_json = match input.evaluated_json {
            Some(value) => Some(crate::models::json_string_required(&value)?),
            None => existing.evaluated_json,
        };
        let duration_ms = input.duration_ms.or(existing.duration_ms);

        let result = sqlx::query(
            r#"
            UPDATE attack_results
            SET success = ?, response_json = ?, evaluated_json = ?, duration_ms = ?
            WHERE id = ?
            "#,
        )
        .bind(success)
        .bind(&response_json)
        .bind(&evaluated_json)
        .bind(duration_ms)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_storage()?;

        ensure_rows_affected(result, "attack_result")?;
        self.get(id).await
    }

    async fn delete(&self, id: &str) -> PromptLabResult<()> {
        let result = sqlx::query("DELETE FROM attack_results WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_storage()?;

        ensure_rows_affected(result, "attack_result")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CreateProject, CreateScan};
    use crate::pool::test_utils::test_database;
    use crate::repositories::{ProjectRepository, ScanRepository};

    #[tokio::test]
    async fn attack_result_records_probe_outcome() {
        let db = test_database().await;
        let repos = db.repositories();

        let project = repos
            .projects()
            .create(CreateProject {
                name: "proj".into(),
                description: None,
            })
            .await
            .unwrap();

        let scan = repos
            .scans()
            .create(CreateScan {
                project_id: project.id,
                target_id: None,
                name: "scan".into(),
                status: None,
                playbook_json: None,
            })
            .await
            .unwrap();

        let result = repos
            .attack_results()
            .create(CreateAttackResult {
                scan_id: scan.id.clone(),
                payload_id: None,
                target_id: None,
                probe_id: Some("probe-1".into()),
                success: true,
                response_json: Some(serde_json::json!({"text": "leaked"})),
                evaluated_json: None,
                duration_ms: Some(42),
            })
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(repos.attack_results().list_by_scan(&scan.id).await.unwrap().len(), 1);
    }
}
