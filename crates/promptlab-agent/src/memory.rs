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

/// True when the category outcome should be treated as a durable failure for retry context.
pub fn is_attack_failure_outcome(
    stopped_reason: &str,
    endpoint_unhealthy: bool,
    endpoint_error: Option<&str>,
) -> bool {
    if endpoint_unhealthy {
        return true;
    }
    if endpoint_error.is_some_and(|e| !e.trim().is_empty()) {
        return true;
    }
    let reason = stopped_reason.to_ascii_lowercase();
    reason.contains("fail")
        || reason.contains("error")
        || reason.contains("endpoint issue")
        || reason.contains("timeout")
        || reason.contains("cancelled")
        || reason.contains("recover")
}

/// Persist category outcome to Scan + Target LTM so Retry on the same target can reload it.
pub async fn remember_attack_category_outcome(
    store: Option<&dyn AgentMemoryStore>,
    ctx: &MemoryContext,
    agent_id: AgentId,
    category: &str,
    stopped_reason: &str,
    content: String,
    content_json: Value,
    importance: f64,
    endpoint_unhealthy: bool,
    endpoint_error: Option<&str>,
) {
    let Some(store) = store else { return };
    let category = category.trim();
    if category.is_empty() {
        return;
    }

    let mut scopes: Vec<(MemoryScopeType, String)> = Vec::new();
    if let Some(scan_id) = ctx.scan_id.as_ref().filter(|s| !s.is_empty()) {
        scopes.push((MemoryScopeType::Scan, scan_id.clone()));
    }
    if let Some(target_id) = ctx.target_id.as_ref().filter(|s| !s.is_empty()) {
        scopes.push((MemoryScopeType::Target, target_id.clone()));
    }
    if scopes.is_empty() {
        scopes.push(ctx.primary_scope());
    }

    let outcome_key = format!("attack.{category}.last_outcome");
    for (scope_type, scope_id) in &scopes {
        remember_ltm(
            Some(store),
            LtmWrite {
                agent_id,
                scope_type: *scope_type,
                scope_id: scope_id.clone(),
                memory_key: outcome_key.clone(),
                content: content.clone(),
                content_json: Some(content_json.clone()),
                importance,
            },
        )
        .await;
    }

    if !is_attack_failure_outcome(stopped_reason, endpoint_unhealthy, endpoint_error) {
        return;
    }

    // Target-scoped failure key — primary source for the next Retry Scan on this target.
    if let Some(target_id) = ctx.target_id.as_ref().filter(|s| !s.is_empty()) {
        let failure_content = format!(
            "PRIOR SCAN FAILURE category={category} reason={stopped_reason} unhealthy={endpoint_unhealthy} err={} | {content}",
            endpoint_error.unwrap_or("-")
        );
        let mut failure_json = content_json.clone();
        if let Some(obj) = failure_json.as_object_mut() {
            obj.insert("failure".into(), Value::Bool(true));
            obj.insert("stopped_reason".into(), Value::String(stopped_reason.into()));
            obj.insert(
                "endpoint_unhealthy".into(),
                Value::Bool(endpoint_unhealthy),
            );
            if let Some(err) = endpoint_error {
                obj.insert("endpoint_error".into(), Value::String(err.into()));
            }
        }
        remember_ltm(
            Some(store),
            LtmWrite {
                agent_id,
                scope_type: MemoryScopeType::Target,
                scope_id: target_id.clone(),
                memory_key: format!("attack.{category}.last_failure"),
                content: failure_content,
                content_json: Some(failure_json),
                importance: importance.max(0.9),
            },
        )
        .await;
    }
}

/// Load prior category failure/outcome for Retry context (Target + Scan scopes).
pub async fn load_prior_attack_failure_block(
    store: Option<&dyn AgentMemoryStore>,
    ctx: &MemoryContext,
    agent_id: AgentId,
    category: &str,
) -> String {
    let Some(store) = store else {
        return String::new();
    };
    let category = category.trim();
    if category.is_empty() {
        return String::new();
    }

    let keys = [
        format!("attack.{category}.last_failure"),
        format!("attack.{category}.last_outcome"),
    ];
    // Same-strategy first, then sibling execution agent (user may switch Sequential↔Agentic).
    let agents: Vec<AgentId> = match agent_id {
        AgentId::SequentialAttackExecution => vec![
            AgentId::SequentialAttackExecution,
            AgentId::AgenticAttackExecution,
        ],
        AgentId::AgenticAttackExecution => vec![
            AgentId::AgenticAttackExecution,
            AgentId::SequentialAttackExecution,
        ],
        other => vec![other],
    };

    let mut scopes: Vec<(MemoryScopeType, String)> = Vec::new();
    if let Some(target_id) = ctx.target_id.as_ref().filter(|s| !s.is_empty()) {
        scopes.push((MemoryScopeType::Target, target_id.clone()));
    }
    if let Some(scan_id) = ctx.scan_id.as_ref().filter(|s| !s.is_empty()) {
        scopes.push((MemoryScopeType::Scan, scan_id.clone()));
    }
    if scopes.is_empty() {
        scopes.push(ctx.primary_scope());
    }

    let mut lines: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::<String>::new();

    for (scope_type, scope_id) in scopes {
        for agent in &agents {
            match store
                .ltm_list(Some(agent.as_str()), scope_type, &scope_id, 24)
                .await
            {
                Ok(entries) => {
                    for entry in entries {
                        if !keys.iter().any(|k| k == &entry.memory_key) {
                            continue;
                        }
                        let dedupe = format!("{}:{}:{}", agent.as_str(), entry.memory_key, entry.content);
                        if !seen.insert(dedupe) {
                            continue;
                        }
                        lines.push(format!(
                            "- [{}|{}|{}] {}",
                            agent.as_str(),
                            scope_type.as_str(),
                            entry.memory_key,
                            truncate(&entry.content, 320)
                        ));
                    }
                }
                Err(err) => warn!(error = %err, "prior attack failure LTM read failed"),
            }
        }
    }

    if lines.is_empty() {
        return String::new();
    }

    let mut out = String::from(
        "Prior attack failure context (use on Retry — adjust pacing/recover before repeating the same error):\n",
    );
    for line in lines.into_iter().take(6) {
        out.push_str(&line);
        out.push('\n');
    }
    out.push('\n');
    out
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

    let mut scopes: Vec<(MemoryScopeType, String)> = Vec::new();
    let primary = ctx.primary_scope();
    scopes.push(primary.clone());
    if let Some(scan_id) = ctx.scan_id.as_ref().filter(|s| !s.is_empty()) {
        let scan_scope = (MemoryScopeType::Scan, scan_id.clone());
        if scan_scope != primary {
            scopes.push(scan_scope);
        }
    }

    let mut ltm_lines: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for (scope_type, scope_id) in &scopes {
        match store
            .ltm_list(Some(agent_id.as_str()), *scope_type, scope_id, 12)
            .await
        {
            Ok(entries) => {
                for entry in entries {
                    let dedupe = format!("{}:{}", entry.memory_key, entry.content);
                    if seen.insert(dedupe) {
                        ltm_lines.push(format!(
                            "- [{}|{}] {}",
                            scope_type.as_str(),
                            entry.memory_key,
                            truncate(&entry.content, 220)
                        ));
                    }
                }
            }
            Err(err) => warn!(error = %err, "agent LTM read failed"),
        }
    }
    if !ltm_lines.is_empty() {
        out.push_str("Long-term memory (durable facts):\n");
        for line in ltm_lines.into_iter().take(16) {
            out.push_str(&line);
            out.push('\n');
        }
        out.push('\n');
    }

    // Also pull Yazg-scoped LTM for sub-agents.
    if agent_id != AgentId::Yazg {
        let mut yazg_lines: Vec<String> = Vec::new();
        let mut yazg_seen = std::collections::HashSet::<String>::new();
        for (scope_type, scope_id) in &scopes {
            match store
                .ltm_list(Some(AgentId::Yazg.as_str()), *scope_type, scope_id, 8)
                .await
            {
                Ok(entries) => {
                    for entry in entries {
                        let dedupe = format!("{}:{}", entry.memory_key, entry.content);
                        if yazg_seen.insert(dedupe) {
                            yazg_lines.push(format!(
                                "- [{}|{}] {}",
                                scope_type.as_str(),
                                entry.memory_key,
                                truncate(&entry.content, 220)
                            ));
                        }
                    }
                }
                Err(err) => warn!(error = %err, "agent shared LTM read failed"),
            }
        }
        if !yazg_lines.is_empty() {
            out.push_str("Long-term memory (Yazg shared):\n");
            for line in yazg_lines.into_iter().take(8) {
                out.push_str(&line);
                out.push('\n');
            }
            out.push('\n');
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

    #[test]
    fn failure_outcome_detects_endpoint_issues() {
        assert!(is_attack_failure_outcome(
            "completed with endpoint issues after 3 recover(ies)",
            true,
            None
        ));
        assert!(is_attack_failure_outcome(
            "attack_failed: timeout",
            false,
            Some("timeout")
        ));
        assert!(!is_attack_failure_outcome(
            "vulnerability confirmed",
            false,
            None
        ));
    }
}
