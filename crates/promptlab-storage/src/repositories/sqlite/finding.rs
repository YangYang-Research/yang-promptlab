use async_trait::async_trait;
use sqlx::SqlitePool;

use promptlab_core::PromptLabResult;

use crate::error::StorageResultExt;
use crate::models::{CreateFinding, Finding, UpdateFinding};
use crate::repositories::FindingRepository;
use crate::util::{ensure_rows_affected, new_id, now};

#[derive(Clone)]
pub struct SqliteFindingRepository {
    pool: SqlitePool,
}

impl SqliteFindingRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl FindingRepository for SqliteFindingRepository {
    async fn create(&self, input: CreateFinding) -> PromptLabResult<Finding> {
        let id = new_id();
        let timestamp = now();
        let status = input.status.unwrap_or_else(|| "open".to_string());
        let evidence_json = crate::models::json_string(&input.evidence_json)?;

        sqlx::query(
            r#"
            INSERT INTO findings (
                id, scan_id, project_id, target_id, title, severity, category,
                description, evidence_json, status, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&input.scan_id)
        .bind(&input.project_id)
        .bind(&input.target_id)
        .bind(&input.title)
        .bind(&input.severity)
        .bind(&input.category)
        .bind(&input.description)
        .bind(&evidence_json)
        .bind(&status)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_storage()?;

        self.get(&id).await
    }

    async fn get(&self, id: &str) -> PromptLabResult<Finding> {
        sqlx::query_as::<_, Finding>("SELECT * FROM findings WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_storage()
    }

    async fn list_by_scan(&self, scan_id: &str) -> PromptLabResult<Vec<Finding>> {
        sqlx::query_as::<_, Finding>(
            "SELECT * FROM findings WHERE scan_id = ? ORDER BY created_at DESC",
        )
        .bind(scan_id)
        .fetch_all(&self.pool)
        .await
        .map_storage()
    }

    async fn list_by_project(&self, project_id: &str) -> PromptLabResult<Vec<Finding>> {
        sqlx::query_as::<_, Finding>(
            "SELECT * FROM findings WHERE project_id = ? ORDER BY created_at DESC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_storage()
    }

    async fn search(&self, query: &str, limit: i64) -> PromptLabResult<Vec<Finding>> {
        let fts_query = query
            .split_whitespace()
            .map(|term| format!("\"{term}\""))
            .collect::<Vec<_>>()
            .join(" AND ");

        sqlx::query_as::<_, Finding>(
            r#"
            SELECT f.*
            FROM findings f
            INNER JOIN findings_fts fts ON f.rowid = fts.rowid
            WHERE findings_fts MATCH ?
            ORDER BY rank
            LIMIT ?
            "#,
        )
        .bind(fts_query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_storage()
    }

    async fn update(&self, id: &str, input: UpdateFinding) -> PromptLabResult<Finding> {
        let existing = self.get(id).await?;
        let title = input.title.unwrap_or(existing.title);
        let severity = input.severity.unwrap_or(existing.severity);
        let category = input.category.or(existing.category);
        let description = input.description.or(existing.description);
        let evidence_json = match input.evidence_json {
            Some(value) => Some(crate::models::json_string_required(&value)?),
            None => existing.evidence_json,
        };
        let status = input.status.unwrap_or(existing.status);
        let updated_at = now();

        let result = sqlx::query(
            r#"
            UPDATE findings
            SET title = ?, severity = ?, category = ?, description = ?,
                evidence_json = ?, status = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&title)
        .bind(&severity)
        .bind(&category)
        .bind(&description)
        .bind(&evidence_json)
        .bind(&status)
        .bind(updated_at)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_storage()?;

        ensure_rows_affected(result, "finding")?;
        self.get(id).await
    }

    async fn delete(&self, id: &str) -> PromptLabResult<()> {
        let result = sqlx::query("DELETE FROM findings WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_storage()?;

        ensure_rows_affected(result, "finding")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CreateProject, CreateScan};
    use crate::pool::test_utils::test_database;
    use crate::repositories::{ProjectRepository, ScanRepository};

    #[tokio::test]
    async fn finding_fts_search() {
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
                project_id: project.id.clone(),
                target_id: None,
                name: "scan".into(),
                status: None,
                playbook_json: None,
            })
            .await
            .unwrap();

        repos
            .findings()
            .create(CreateFinding {
                scan_id: scan.id,
                project_id: project.id,
                target_id: None,
                title: "Prompt injection detected".into(),
                severity: "high".into(),
                category: Some("injection".into()),
                description: Some("System prompt leak via delimiter attack".into()),
                evidence_json: None,
                status: None,
            })
            .await
            .unwrap();

        let hits = repos
            .findings()
            .search("injection", 10)
            .await
            .unwrap();

        assert_eq!(hits.len(), 1);
    }
}
