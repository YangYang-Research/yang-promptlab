use async_trait::async_trait;
use sqlx::SqlitePool;

use aisec_core::AisecResult;

use crate::error::StorageResultExt;
use crate::models::{CreateTarget, Target, UpdateTarget};
use crate::repositories::TargetRepository;
use crate::util::{ensure_rows_affected, new_id, now};

#[derive(Clone)]
pub struct SqliteTargetRepository {
    pool: SqlitePool,
}

impl SqliteTargetRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TargetRepository for SqliteTargetRepository {
    async fn create(&self, input: CreateTarget) -> AisecResult<Target> {
        let id = new_id();
        let timestamp = now();
        let descriptor_json = match &input.descriptor_json {
            Some(value) => crate::models::json_string_required(value)?,
            None => "{}".to_string(),
        };

        sqlx::query(
            r#"
            INSERT INTO targets (id, project_id, name, target_type, descriptor_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&input.project_id)
        .bind(&input.name)
        .bind(&input.target_type)
        .bind(&descriptor_json)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_storage()?;

        self.get(&id).await
    }

    async fn get(&self, id: &str) -> AisecResult<Target> {
        sqlx::query_as::<_, Target>("SELECT * FROM targets WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_storage()
    }

    async fn list_by_project(&self, project_id: &str) -> AisecResult<Vec<Target>> {
        sqlx::query_as::<_, Target>(
            "SELECT * FROM targets WHERE project_id = ? ORDER BY created_at DESC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_storage()
    }

    async fn update(&self, id: &str, input: UpdateTarget) -> AisecResult<Target> {
        let existing = self.get(id).await?;
        let name = input.name.unwrap_or(existing.name);
        let target_type = input.target_type.unwrap_or(existing.target_type);
        let descriptor_json = match input.descriptor_json {
            Some(value) => crate::models::json_string_required(&value)?,
            None => existing.descriptor_json,
        };
        let updated_at = now();

        let result = sqlx::query(
            r#"
            UPDATE targets
            SET name = ?, target_type = ?, descriptor_json = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&name)
        .bind(&target_type)
        .bind(&descriptor_json)
        .bind(updated_at)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_storage()?;

        ensure_rows_affected(result, "target")?;
        self.get(id).await
    }

    async fn delete(&self, id: &str) -> AisecResult<()> {
        let result = sqlx::query("DELETE FROM targets WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_storage()?;

        ensure_rows_affected(result, "target")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CreateProject;
    use crate::pool::test_utils::test_database;
    use crate::repositories::ProjectRepository;

    #[tokio::test]
    async fn target_crud() {
        let db = test_database().await;
        let projects = db.repositories().projects();
        let targets = db.repositories().targets();

        let project = projects
            .create(CreateProject {
                name: "proj".into(),
                description: None,
            })
            .await
            .unwrap();

        let target = targets
            .create(CreateTarget {
                project_id: project.id.clone(),
                name: "OpenAI API".into(),
                target_type: "llm".into(),
                descriptor_json: Some(serde_json::json!({"url": "https://api.example.com"})),
            })
            .await
            .unwrap();

        let listed = targets.list_by_project(&project.id).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, target.id);

        targets.delete(&target.id).await.unwrap();
    }
}
