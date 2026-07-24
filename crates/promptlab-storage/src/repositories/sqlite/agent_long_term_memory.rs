use async_trait::async_trait;
use sqlx::SqlitePool;

use promptlab_core::PromptLabResult;

use crate::error::StorageResultExt;
use crate::models::{
    json_string, AgentLongTermMemory, UpdateAgentLongTermMemory, UpsertAgentLongTermMemory,
};
use crate::repositories::AgentLongTermMemoryRepository;
use crate::util::{ensure_rows_affected, new_id, now};

#[derive(Clone)]
pub struct SqliteAgentLongTermMemoryRepository {
    pool: SqlitePool,
}

impl SqliteAgentLongTermMemoryRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AgentLongTermMemoryRepository for SqliteAgentLongTermMemoryRepository {
    async fn upsert(&self, input: UpsertAgentLongTermMemory) -> PromptLabResult<AgentLongTermMemory> {
        let id = new_id();
        let timestamp = now();
        let importance = input.importance.unwrap_or(0.5).clamp(0.0, 1.0);
        let content_json = json_string(&input.content_json)?;
        let scope_id = if input.scope_type == "global" {
            String::new()
        } else {
            input.scope_id
        };

        sqlx::query(
            r#"
            INSERT INTO agent_long_term_memory (
                id, agent_id, scope_type, scope_id, memory_key,
                content, content_json, importance, access_count,
                last_accessed_at, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, NULL, ?, ?)
            ON CONFLICT(agent_id, scope_type, scope_id, memory_key) DO UPDATE SET
                content = excluded.content,
                content_json = excluded.content_json,
                importance = excluded.importance,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&id)
        .bind(&input.agent_id)
        .bind(&input.scope_type)
        .bind(&scope_id)
        .bind(&input.memory_key)
        .bind(&input.content)
        .bind(&content_json)
        .bind(importance)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_storage()?;

        self.get_by_key(
            &input.agent_id,
            &input.scope_type,
            &scope_id,
            &input.memory_key,
        )
        .await
    }

    async fn get(&self, id: &str) -> PromptLabResult<AgentLongTermMemory> {
        sqlx::query_as::<_, AgentLongTermMemory>(
            "SELECT * FROM agent_long_term_memory WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_storage()
    }

    async fn get_by_key(
        &self,
        agent_id: &str,
        scope_type: &str,
        scope_id: &str,
        memory_key: &str,
    ) -> PromptLabResult<AgentLongTermMemory> {
        let resolved_scope = if scope_type == "global" { "" } else { scope_id };
        sqlx::query_as::<_, AgentLongTermMemory>(
            r#"
            SELECT * FROM agent_long_term_memory
            WHERE agent_id = ? AND scope_type = ? AND scope_id = ? AND memory_key = ?
            "#,
        )
        .bind(agent_id)
        .bind(scope_type)
        .bind(resolved_scope)
        .bind(memory_key)
        .fetch_one(&self.pool)
        .await
        .map_storage()
    }

    async fn list_by_scope(
        &self,
        scope_type: &str,
        scope_id: &str,
    ) -> PromptLabResult<Vec<AgentLongTermMemory>> {
        let resolved_scope = if scope_type == "global" { "" } else { scope_id };
        sqlx::query_as::<_, AgentLongTermMemory>(
            r#"
            SELECT * FROM agent_long_term_memory
            WHERE scope_type = ? AND scope_id = ?
            ORDER BY importance DESC, updated_at DESC
            "#,
        )
        .bind(scope_type)
        .bind(resolved_scope)
        .fetch_all(&self.pool)
        .await
        .map_storage()
    }

    async fn list_by_agent_scope(
        &self,
        agent_id: &str,
        scope_type: &str,
        scope_id: &str,
    ) -> PromptLabResult<Vec<AgentLongTermMemory>> {
        let resolved_scope = if scope_type == "global" { "" } else { scope_id };
        sqlx::query_as::<_, AgentLongTermMemory>(
            r#"
            SELECT * FROM agent_long_term_memory
            WHERE agent_id = ? AND scope_type = ? AND scope_id = ?
            ORDER BY importance DESC, updated_at DESC
            "#,
        )
        .bind(agent_id)
        .bind(scope_type)
        .bind(resolved_scope)
        .fetch_all(&self.pool)
        .await
        .map_storage()
    }

    async fn update(
        &self,
        id: &str,
        input: UpdateAgentLongTermMemory,
    ) -> PromptLabResult<AgentLongTermMemory> {
        let existing = self.get(id).await?;
        let content = input.content.unwrap_or(existing.content);
        let content_json = match input.content_json {
            Some(value) => Some(crate::models::json_string_required(&value)?),
            None => existing.content_json,
        };
        let importance = input
            .importance
            .unwrap_or(existing.importance)
            .clamp(0.0, 1.0);
        let updated_at = now();

        let result = sqlx::query(
            r#"
            UPDATE agent_long_term_memory
            SET content = ?, content_json = ?, importance = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&content)
        .bind(&content_json)
        .bind(importance)
        .bind(updated_at)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_storage()?;

        ensure_rows_affected(result, "agent long-term memory")?;
        self.get(id).await
    }

    async fn touch(&self, id: &str) -> PromptLabResult<AgentLongTermMemory> {
        let updated_at = now();
        let result = sqlx::query(
            r#"
            UPDATE agent_long_term_memory
            SET access_count = access_count + 1,
                last_accessed_at = ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(updated_at)
        .bind(updated_at)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_storage()?;

        ensure_rows_affected(result, "agent long-term memory")?;
        self.get(id).await
    }

    async fn delete(&self, id: &str) -> PromptLabResult<()> {
        let result = sqlx::query("DELETE FROM agent_long_term_memory WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_storage()?;

        ensure_rows_affected(result, "agent long-term memory")
    }

    async fn delete_by_scope(&self, scope_type: &str, scope_id: &str) -> PromptLabResult<u64> {
        let resolved_scope = if scope_type == "global" { "" } else { scope_id };
        let result = sqlx::query(
            "DELETE FROM agent_long_term_memory WHERE scope_type = ? AND scope_id = ?",
        )
        .bind(scope_type)
        .bind(resolved_scope)
        .execute(&self.pool)
        .await
        .map_storage()?;

        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CreateProject;
    use crate::pool::test_utils::test_database;
    use crate::repositories::ProjectRepository;

    #[tokio::test]
    async fn long_term_memory_upsert_and_touch() {
        let db = test_database().await;
        let repos = db.repositories();
        let project = repos
            .projects()
            .create(CreateProject {
                name: "LTM".into(),
                description: None,
            })
            .await
            .unwrap();

        let ltm = repos.agent_long_term_memory();
        let created = ltm
            .upsert(UpsertAgentLongTermMemory {
                agent_id: "yazg".into(),
                scope_type: "project".into(),
                scope_id: project.id.clone(),
                memory_key: "preferred_profile".into(),
                content: "standard".into(),
                content_json: None,
                importance: Some(0.9),
            })
            .await
            .unwrap();

        let updated = ltm
            .upsert(UpsertAgentLongTermMemory {
                agent_id: "yazg".into(),
                scope_type: "project".into(),
                scope_id: project.id.clone(),
                memory_key: "preferred_profile".into(),
                content: "deep".into(),
                content_json: Some(serde_json::json!({"mode": "deep"})),
                importance: Some(0.95),
            })
            .await
            .unwrap();

        assert_eq!(created.id, updated.id);
        assert_eq!(updated.content, "deep");
        assert_eq!(updated.access_count, 0);

        let touched = ltm.touch(&updated.id).await.unwrap();
        assert_eq!(touched.access_count, 1);
        assert!(touched.last_accessed_at.is_some());

        assert_eq!(
            ltm.list_by_agent_scope("yazg", "project", &project.id)
                .await
                .unwrap()
                .len(),
            1
        );
    }
}
