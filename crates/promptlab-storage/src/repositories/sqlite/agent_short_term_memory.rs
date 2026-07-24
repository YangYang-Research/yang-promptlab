use async_trait::async_trait;
use sqlx::SqlitePool;
use time::OffsetDateTime;

use promptlab_core::PromptLabResult;

use crate::error::StorageResultExt;
use crate::models::{
    json_string, AgentShortTermMemory, CreateAgentShortTermMemory,
};
use crate::repositories::AgentShortTermMemoryRepository;
use crate::util::{ensure_rows_affected, new_id, now};

#[derive(Clone)]
pub struct SqliteAgentShortTermMemoryRepository {
    pool: SqlitePool,
}

impl SqliteAgentShortTermMemoryRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AgentShortTermMemoryRepository for SqliteAgentShortTermMemoryRepository {
    async fn create(&self, input: CreateAgentShortTermMemory) -> PromptLabResult<AgentShortTermMemory> {
        let id = new_id();
        let timestamp = now();
        let importance = input.importance.unwrap_or(0.5).clamp(0.0, 1.0);
        let content_json = json_string(&input.content_json)?;

        sqlx::query(
            r#"
            INSERT INTO agent_short_term_memory (
                id, session_id, agent_id, project_id, target_id, scan_id,
                role, memory_key, content, content_json, importance,
                expires_at, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&input.session_id)
        .bind(&input.agent_id)
        .bind(&input.project_id)
        .bind(&input.target_id)
        .bind(&input.scan_id)
        .bind(&input.role)
        .bind(&input.memory_key)
        .bind(&input.content)
        .bind(&content_json)
        .bind(importance)
        .bind(input.expires_at)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_storage()?;

        self.get(&id).await
    }

    async fn get(&self, id: &str) -> PromptLabResult<AgentShortTermMemory> {
        sqlx::query_as::<_, AgentShortTermMemory>(
            "SELECT * FROM agent_short_term_memory WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_storage()
    }

    async fn list_by_session(&self, session_id: &str) -> PromptLabResult<Vec<AgentShortTermMemory>> {
        sqlx::query_as::<_, AgentShortTermMemory>(
            r#"
            SELECT * FROM agent_short_term_memory
            WHERE session_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .map_storage()
    }

    async fn list_by_session_agent(
        &self,
        session_id: &str,
        agent_id: &str,
    ) -> PromptLabResult<Vec<AgentShortTermMemory>> {
        sqlx::query_as::<_, AgentShortTermMemory>(
            r#"
            SELECT * FROM agent_short_term_memory
            WHERE session_id = ? AND agent_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(session_id)
        .bind(agent_id)
        .fetch_all(&self.pool)
        .await
        .map_storage()
    }

    async fn delete(&self, id: &str) -> PromptLabResult<()> {
        let result = sqlx::query("DELETE FROM agent_short_term_memory WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_storage()?;

        ensure_rows_affected(result, "agent short-term memory")
    }

    async fn delete_by_session(&self, session_id: &str) -> PromptLabResult<u64> {
        let result = sqlx::query("DELETE FROM agent_short_term_memory WHERE session_id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_storage()?;

        Ok(result.rows_affected())
    }

    async fn prune_expired(&self, cutoff: OffsetDateTime) -> PromptLabResult<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM agent_short_term_memory
            WHERE expires_at IS NOT NULL AND expires_at <= ?
            "#,
        )
        .bind(cutoff)
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
    async fn short_term_memory_session_roundtrip() {
        let db = test_database().await;
        let repos = db.repositories();
        let project = repos
            .projects()
            .create(CreateProject {
                name: "Mem".into(),
                description: None,
            })
            .await
            .unwrap();

        let stm = repos.agent_short_term_memory();
        let created = stm
            .create(CreateAgentShortTermMemory {
                session_id: "sess-1".into(),
                agent_id: "yazg".into(),
                project_id: Some(project.id.clone()),
                target_id: None,
                scan_id: None,
                role: "observation".into(),
                memory_key: Some("last_intent".into()),
                content: "attack_plan".into(),
                content_json: Some(serde_json::json!({"intent": "attack_plan"})),
                importance: Some(0.8),
                expires_at: None,
            })
            .await
            .unwrap();

        assert_eq!(created.session_id, "sess-1");
        assert_eq!(stm.list_by_session("sess-1").await.unwrap().len(), 1);
        assert_eq!(
            stm.list_by_session_agent("sess-1", "yazg")
                .await
                .unwrap()
                .len(),
            1
        );

        let deleted = stm.delete_by_session("sess-1").await.unwrap();
        assert_eq!(deleted, 1);
        assert!(stm.list_by_session("sess-1").await.unwrap().is_empty());
    }
}
