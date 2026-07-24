use async_trait::async_trait;
use sqlx::SqlitePool;

use aisec_core::AisecResult;

use crate::error::StorageResultExt;
use crate::models::{CreateReport, Report, UpdateReport};
use crate::repositories::ReportRepository;
use crate::util::{ensure_rows_affected, new_id, now};

#[derive(Clone)]
pub struct SqliteReportRepository {
    pool: SqlitePool,
}

impl SqliteReportRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ReportRepository for SqliteReportRepository {
    async fn create(&self, input: CreateReport) -> AisecResult<Report> {
        let id = new_id();
        let timestamp = now();
        let status = input.status.unwrap_or_else(|| "pending".to_string());
        let metadata_json = crate::models::json_string(&input.metadata_json)?;

        sqlx::query(
            r#"
            INSERT INTO reports (
                id, project_id, scan_id, name, format, status, file_path,
                metadata_json, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&input.project_id)
        .bind(&input.scan_id)
        .bind(&input.name)
        .bind(&input.format)
        .bind(&status)
        .bind(&input.file_path)
        .bind(&metadata_json)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_storage()?;

        self.get(&id).await
    }

    async fn get(&self, id: &str) -> AisecResult<Report> {
        sqlx::query_as::<_, Report>("SELECT * FROM reports WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_storage()
    }

    async fn list_by_project(&self, project_id: &str) -> AisecResult<Vec<Report>> {
        sqlx::query_as::<_, Report>(
            "SELECT * FROM reports WHERE project_id = ? ORDER BY created_at DESC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_storage()
    }

    async fn update(&self, id: &str, input: UpdateReport) -> AisecResult<Report> {
        let existing = self.get(id).await?;
        let name = input.name.unwrap_or(existing.name);
        let format = input.format.unwrap_or(existing.format);
        let status = input.status.unwrap_or(existing.status);
        let file_path = input.file_path.or(existing.file_path);
        let metadata_json = match input.metadata_json {
            Some(value) => Some(crate::models::json_string_required(&value)?),
            None => existing.metadata_json,
        };
        let updated_at = now();

        let result = sqlx::query(
            r#"
            UPDATE reports
            SET name = ?, format = ?, status = ?, file_path = ?,
                metadata_json = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&name)
        .bind(&format)
        .bind(&status)
        .bind(&file_path)
        .bind(&metadata_json)
        .bind(updated_at)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_storage()?;

        ensure_rows_affected(result, "report")?;
        self.get(id).await
    }

    async fn delete(&self, id: &str) -> AisecResult<()> {
        let result = sqlx::query("DELETE FROM reports WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_storage()?;

        ensure_rows_affected(result, "report")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CreateProject;
    use crate::pool::test_utils::test_database;
    use crate::repositories::ProjectRepository;

    #[tokio::test]
    async fn report_crud() {
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

        let report = repos
            .reports()
            .create(CreateReport {
                project_id: project.id.clone(),
                scan_id: None,
                name: "Executive Summary".into(),
                format: "pdf".into(),
                status: None,
                file_path: None,
                metadata_json: None,
            })
            .await
            .unwrap();

        assert_eq!(repos.reports().list_by_project(&project.id).await.unwrap().len(), 1);
        repos.reports().delete(&report.id).await.unwrap();
    }
}
