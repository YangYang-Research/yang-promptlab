use async_trait::async_trait;
use sqlx::SqlitePool;

use aisec_core::AisecResult;

use crate::error::StorageResultExt;
use crate::models::{CreateScan, Scan, UpdateScan};
use crate::repositories::ScanRepository;
use crate::util::{ensure_rows_affected, new_id, now};

#[derive(Clone)]
pub struct SqliteScanRepository {
    pool: SqlitePool,
}

impl SqliteScanRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ScanRepository for SqliteScanRepository {
    async fn create(&self, input: CreateScan) -> AisecResult<Scan> {
        let id = new_id();
        let timestamp = now();
        let status = input.status.unwrap_or_else(|| "pending".to_string());
        let playbook_json = crate::models::json_string(&input.playbook_json)?;

        sqlx::query(
            r#"
            INSERT INTO scans (
                id, project_id, target_id, name, status, playbook_json,
                started_at, completed_at, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, NULL, NULL, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&input.project_id)
        .bind(&input.target_id)
        .bind(&input.name)
        .bind(&status)
        .bind(&playbook_json)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_storage()?;

        self.get(&id).await
    }

    async fn get(&self, id: &str) -> AisecResult<Scan> {
        sqlx::query_as::<_, Scan>("SELECT * FROM scans WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_storage()
    }

    async fn list_by_project(&self, project_id: &str) -> AisecResult<Vec<Scan>> {
        sqlx::query_as::<_, Scan>(
            "SELECT * FROM scans WHERE project_id = ? ORDER BY created_at DESC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_storage()
    }

    async fn update(&self, id: &str, input: UpdateScan) -> AisecResult<Scan> {
        let existing = self.get(id).await?;
        let target_id = match input.target_id {
            Some(value) => value,
            None => existing.target_id,
        };
        let name = input.name.unwrap_or(existing.name);
        let status = input.status.unwrap_or(existing.status);
        let playbook_json = match input.playbook_json {
            Some(value) => Some(crate::models::json_string_required(&value)?),
            None => existing.playbook_json,
        };
        let started_at = match input.started_at {
            Some(value) => value,
            None => existing.started_at,
        };
        let completed_at = match input.completed_at {
            Some(value) => value,
            None => existing.completed_at,
        };
        let updated_at = now();

        let result = sqlx::query(
            r#"
            UPDATE scans
            SET target_id = ?, name = ?, status = ?, playbook_json = ?,
                started_at = ?, completed_at = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&target_id)
        .bind(&name)
        .bind(&status)
        .bind(&playbook_json)
        .bind(started_at)
        .bind(completed_at)
        .bind(updated_at)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_storage()?;

        ensure_rows_affected(result, "scan")?;
        self.get(id).await
    }

    async fn delete(&self, id: &str) -> AisecResult<()> {
        let result = sqlx::query("DELETE FROM scans WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_storage()?;

        ensure_rows_affected(result, "scan")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CreateProject;
    use crate::pool::test_utils::test_database;
    use crate::repositories::ProjectRepository;

    #[tokio::test]
    async fn scan_lifecycle() {
        let db = test_database().await;
        let projects = db.repositories().projects();
        let scans = db.repositories().scans();

        let project = projects
            .create(CreateProject {
                name: "proj".into(),
                description: None,
            })
            .await
            .unwrap();

        let scan = scans
            .create(CreateScan {
                project_id: project.id,
                target_id: None,
                name: "baseline".into(),
                status: None,
                playbook_json: Some(serde_json::json!({"playbook": "owasp-llm"})),
            })
            .await
            .unwrap();

        assert_eq!(scan.status, "pending");

        let updated = scans
            .update(
                &scan.id,
                UpdateScan {
                    status: Some("running".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(updated.status, "running");
    }
}
