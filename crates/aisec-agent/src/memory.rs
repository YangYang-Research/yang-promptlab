//! Agent short-term / long-term memory contracts.
//!
//! Host (desktop) implements [`AgentMemoryStore`] against SQLite.
//! Agents treat memory failures as soft — never fail the turn.

use async_trait::async_trait;
use serde_json::Value;
use tracing::warn;

use crate::types::AgentId;

/// Durable memory scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryScopeType {
    Global,
    Project,
    Target,
    Scan,
}

impl MemoryScopeType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Project => "project",
            Self::Target => "target",
            Self::Scan => "scan",
        }
    }
}

/// Session + entity scope for STM/LTM operations.
#[derive(Debug, Clone, Default)]
pub struct MemoryContext {
    pub session_id: String,
    pub project_id: Option<String>,
    pub target_id: Option<String>,
    pub scan_id: Option<String>,
}

impl MemoryContext {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            ..Self::default()
        }
    }

    pub fn with_project(mut self, project_id: Option<String>) -> Self {
        self.project_id = project_id;
        self
    }

    pub fn with_target(mut self, target_id: Option<String>) -> Self {
        self.target_id = target_id;
        self
    }

    pub fn with_scan(mut self, scan_id: Option<String>) -> Self {
        self.scan_id = scan_id;
        self
    }

    /// Prefer target → project → scan → global for LTM lookups.
    pub fn primary_scope(&self) -> (MemoryScopeType, String) {
        if let Some(id) = self.target_id.as_ref().filter(|s| !s.is_empty()) {
            return (MemoryScopeType::Target, id.clone());
        }
        if let Some(id) = self.project_id.as_ref().filter(|s| !s.is_empty()) {
            return (MemoryScopeType::Project, id.clone());
        }
        if let Some(id) = self.scan_id.as_ref().filter(|s| !s.is_empty()) {
            return (MemoryScopeType::Scan, id.clone());
        }
        (MemoryScopeType::Global, String::new())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StmRole {
    User,
    Assistant,
    System,
    Observation,
    Tool,
}

impl StmRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::System => "system",
            Self::Observation => "observation",
            Self::Tool => "tool",
        }
    }
}

#[derive(Debug, Clone)]
pub struct StmWrite {
    pub agent_id: AgentId,
    pub role: StmRole,
    pub memory_key: Option<String>,
    pub content: String,
    pub content_json: Option<Value>,
    pub importance: f64,
}

#[derive(Debug, Clone)]
pub struct StmEntry {
    pub agent_id: String,
    pub role: String,
    pub memory_key: Option<String>,
    pub content: String,
    pub importance: f64,
}

#[derive(Debug, Clone)]
pub struct LtmWrite {
    pub agent_id: AgentId,
    pub scope_type: MemoryScopeType,
    pub scope_id: String,
    pub memory_key: String,
    pub content: String,
    pub content_json: Option<Value>,
    pub importance: f64,
}

#[derive(Debug, Clone)]
pub struct LtmEntry {
    pub id: String,
    pub agent_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub memory_key: String,
    pub content: String,
    pub importance: f64,
}

/// Host-backed memory persistence for Yazg + sub-agents.
#[async_trait]
pub trait AgentMemoryStore: Send + Sync {
    async fn stm_append(&self, ctx: &MemoryContext, entry: StmWrite) -> Result<(), String>;
    async fn stm_list(
        &self,
        session_id: &str,
        agent_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<StmEntry>, String>;

    async fn ltm_upsert(&self, entry: LtmWrite) -> Result<(), String>;
    async fn ltm_list(
        &self,
        agent_id: Option<&str>,
        scope_type: MemoryScopeType,
        scope_id: &str,
        limit: usize,
    ) -> Result<Vec<LtmEntry>, String>;
}

/// Soft STM append — never fails the agent turn.
pub async fn remember_stm(
    store: Option<&dyn AgentMemoryStore>,
    ctx: &MemoryContext,
    entry: StmWrite,
) {
    let Some(store) = store else { return };
    if ctx.session_id.trim().is_empty() {
        return;
    }
    if let Err(err) = store.stm_append(ctx, entry).await {
        warn!(error = %err, "agent STM write failed");
    }
}

/// Soft LTM upsert — never fails the agent turn.
pub async fn remember_ltm(store: Option<&dyn AgentMemoryStore>, entry: LtmWrite) {
    let Some(store) = store else { return };
    if let Err(err) = store.ltm_upsert(entry).await {
        warn!(error = %err, "agent LTM write failed");
    }
}

/// Load STM + LTM into a prompt block for ReAct / orchestrators.
pub async fn load_memory_prompt_block(
    store: Option<&dyn AgentMemoryStore>,
    ctx: &MemoryContext,
    agent_id: AgentId,
) -> String {
    let Some(store) = store else {
        return String::new();
    };
    if ctx.session_id.trim().is_empty() {
        return String::new();
    }

    let mut out = String::new();

    match store.stm_list(&ctx.session_id, None, 24).await {
        Ok(entries) if !entries.is_empty() => {
            out.push_str("Short-term memory (this session):\n");
            for entry in entries {
                out.push_str(&format!(
                    "- [{}|{}] {}\n",
                    entry.agent_id,
                    entry.role,
                    truncate(&entry.content, 220)
                ));
            }
            out.push('\n');
        }
        Ok(_) => {}
        Err(err) => warn!(error = %err, "agent STM read failed"),
    }

    let (scope_type, scope_id) = ctx.primary_scope();
    match store
        .ltm_list(Some(agent_id.as_str()), scope_type, &scope_id, 12)
        .await
    {
        Ok(entries) if !entries.is_empty() => {
            out.push_str("Long-term memory (durable facts):\n");
            for entry in entries {
                out.push_str(&format!(
                    "- [{}] {}\n",
                    entry.memory_key,
                    truncate(&entry.content, 220)
                ));
            }
            out.push('\n');
        }
        Ok(_) => {}
        Err(err) => warn!(error = %err, "agent LTM read failed"),
    }

    // Also pull Yazg-scoped LTM for sub-agents.
    if agent_id != AgentId::Yazg {
        match store
            .ltm_list(Some(AgentId::Yazg.as_str()), scope_type, &scope_id, 8)
            .await
        {
            Ok(entries) if !entries.is_empty() => {
                out.push_str("Long-term memory (Yazg shared):\n");
                for entry in entries {
                    out.push_str(&format!(
                        "- [{}] {}\n",
                        entry.memory_key,
                        truncate(&entry.content, 220)
                    ));
                }
                out.push('\n');
            }
            Ok(_) => {}
            Err(err) => warn!(error = %err, "agent shared LTM read failed"),
        }
    }

    out
}

fn truncate(s: &str, max: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= max {
        return trimmed.to_string();
    }
    let cut: String = trimmed.chars().take(max.saturating_sub(1)).collect();
    format!("{cut}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_scope_prefers_target() {
        let ctx = MemoryContext::new("s1")
            .with_project(Some("p".into()))
            .with_target(Some("t".into()))
            .with_scan(Some("sc".into()));
        assert_eq!(
            ctx.primary_scope(),
            (MemoryScopeType::Target, "t".into())
        );
    }
}
