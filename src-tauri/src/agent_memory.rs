//! SQLite-backed [`promptlab_agent::AgentMemoryStore`] for Yazg + sub-agents.
//!
//! STM = session event log (AgentCore short-term).
//! LTM = durable scoped facts (AgentCore long-term; extraction is host-side).

use promptlab_agent::{
    AgentMemoryStore, LtmEntry, LtmWrite, MemoryContext, MemoryScopeType, StmEntry, StmSessionSummary,
    StmWrite,
};
use promptlab_storage::{
    AgentLongTermMemoryRepository, AgentShortTermMemoryRepository, CreateAgentShortTermMemory,
    Repositories, UpsertAgentLongTermMemory,
};
use async_trait::async_trait;
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

const STM_DEFAULT_TTL_HOURS: i64 = 24;

#[derive(Clone)]
pub struct SqliteAgentMemoryStore {
    repos: Repositories,
}

impl SqliteAgentMemoryStore {
    pub fn new(repos: Repositories) -> Self {
        Self { repos }
    }
}

fn format_ts(ts: OffsetDateTime) -> Option<String> {
    ts.format(&Rfc3339).ok()
}

fn parse_content_json(raw: Option<String>) -> Option<serde_json::Value> {
    raw.and_then(|s| serde_json::from_str(&s).ok())
}

#[async_trait]
impl AgentMemoryStore for SqliteAgentMemoryStore {
    async fn stm_append(&self, ctx: &MemoryContext, entry: StmWrite) -> Result<(), String> {
        let expires_at = OffsetDateTime::now_utc() + Duration::hours(STM_DEFAULT_TTL_HOURS);
        self.repos
            .agent_short_term_memory()
            .create(CreateAgentShortTermMemory {
                session_id: ctx.session_id.clone(),
                agent_id: entry.agent_id.as_str().to_string(),
                project_id: ctx.project_id.clone(),
                target_id: ctx.target_id.clone(),
                scan_id: ctx.scan_id.clone(),
                role: entry.role.as_str().to_string(),
                memory_key: entry.memory_key,
                content: entry.content,
                content_json: entry.content_json,
                importance: Some(entry.importance),
                expires_at: Some(expires_at),
            })
            .await
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    async fn stm_list(
        &self,
        session_id: &str,
        agent_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<StmEntry>, String> {
        let rows = if let Some(agent_id) = agent_id {
            self.repos
                .agent_short_term_memory()
                .list_by_session_agent(session_id, agent_id)
                .await
        } else {
            self.repos
                .agent_short_term_memory()
                .list_by_session(session_id)
                .await
        }
        .map_err(|err| err.to_string())?;

        Ok(rows
            .into_iter()
            .rev()
            .take(limit)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|row| StmEntry {
                id: row.id,
                agent_id: row.agent_id,
                role: row.role,
                memory_key: row.memory_key,
                content: row.content,
                content_json: parse_content_json(row.content_json),
                importance: row.importance,
                created_at: format_ts(row.created_at),
            })
            .collect())
    }

    async fn stm_list_sessions(
        &self,
        prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<StmSessionSummary>, String> {
        // Opportunistic prune so ListSessions stays clean.
        let _ = self
            .repos
            .agent_short_term_memory()
            .prune_expired(OffsetDateTime::now_utc())
            .await;

        let rows = self
            .repos
            .agent_short_term_memory()
            .list_sessions(prefix, limit)
            .await
            .map_err(|err| err.to_string())?;

        Ok(rows
            .into_iter()
            .map(|row| StmSessionSummary {
                session_id: row.session_id,
                event_count: row.event_count.max(0) as usize,
                first_at: format_ts(row.first_at),
                last_at: format_ts(row.last_at),
            })
            .collect())
    }

    async fn ltm_upsert(&self, entry: LtmWrite) -> Result<(), String> {
        self.repos
            .agent_long_term_memory()
            .upsert(UpsertAgentLongTermMemory {
                agent_id: entry.agent_id.as_str().to_string(),
                scope_type: entry.scope_type.as_str().to_string(),
                scope_id: entry.scope_id,
                memory_key: entry.memory_key,
                content: entry.content,
                content_json: entry.content_json,
                importance: Some(entry.importance),
            })
            .await
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    async fn ltm_list(
        &self,
        agent_id: Option<&str>,
        scope_type: MemoryScopeType,
        scope_id: &str,
        limit: usize,
    ) -> Result<Vec<LtmEntry>, String> {
        let rows = if let Some(agent_id) = agent_id {
            self.repos
                .agent_long_term_memory()
                .list_by_agent_scope(agent_id, scope_type.as_str(), scope_id)
                .await
        } else {
            self.repos
                .agent_long_term_memory()
                .list_by_scope(scope_type.as_str(), scope_id)
                .await
        }
        .map_err(|err| err.to_string())?;

        Ok(rows
            .into_iter()
            .take(limit)
            .map(|row| LtmEntry {
                id: row.id,
                agent_id: row.agent_id,
                scope_type: row.scope_type,
                scope_id: row.scope_id,
                memory_key: row.memory_key,
                content: row.content,
                importance: row.importance,
            })
            .collect())
    }
}
