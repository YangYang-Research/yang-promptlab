use async_trait::async_trait;
use sqlx::SqlitePool;

use promptlab_core::PromptLabResult;

use crate::error::StorageResultExt;
use crate::models::{CreateProject, Project, UpdateProject};
use crate::repositories::ProjectRepository;
use crate::util::{ensure_rows_affected, new_id, now};

#[derive(Clone)]
pub struct SqliteProjectRepository {
    pool: SqlitePool,
}

impl SqliteProjectRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProjectRepository for SqliteProjectRepository {
    async fn create(&self, input: CreateProject) -> PromptLabResult<Project> {
        let id = new_id();
        let timestamp = now();

        sqlx::query(
            r#"
            INSERT INTO projects (id, name, description, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_storage()?;

        self.get(&id).await
    }

    async fn get(&self, id: &str) -> PromptLabResult<Project> {
        sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_storage()
    }

    async fn list(&self) -> PromptLabResult<Vec<Project>> {
        sqlx::query_as::<_, Project>("SELECT * FROM projects ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await
            .map_storage()
    }

    async fn update(&self, id: &str, input: UpdateProject) -> PromptLabResult<Project> {
        let existing = self.get(id).await?;
        let name = input.name.unwrap_or(existing.name);
        let description = input.description.or(existing.description);
        let summary_json = input.summary_json.or(existing.summary_json);
        let updated_at = now();

        let result = sqlx::query(
            r#"
            UPDATE projects
            SET name = ?, description = ?, summary_json = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&name)
        .bind(&description)
        .bind(&summary_json)
        .bind(updated_at)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_storage()?;

        ensure_rows_affected(result, "project")?;
        self.get(id).await
    }

    async fn delete(&self, id: &str) -> PromptLabResult<()> {
        let result = sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_storage()?;

        ensure_rows_affected(result, "project")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::test_utils::test_database;

    #[tokio::test]
    async fn project_crud() {
        let db = test_database().await;
        let repo = db.repositories().projects();

        let created = repo
            .create(CreateProject {
                name: "Pentest Alpha".into(),
                description: Some("demo".into()),
            })
            .await
            .expect("create");

        assert_eq!(created.name, "Pentest Alpha");

        let updated = repo
            .update(
                &created.id,
                UpdateProject {
                    name: Some("Pentest Beta".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("update");

        assert_eq!(updated.name, "Pentest Beta");

        let all = repo.list().await.expect("list");
        assert_eq!(all.len(), 1);

        repo.delete(&created.id).await.expect("delete");
        assert!(repo.get(&created.id).await.is_err());
    }
}
