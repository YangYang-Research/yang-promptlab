//! Agent short-term / long-term memory contracts.
//!
//! Aligned with AWS Bedrock AgentCore Memory types:
//! - **STM** — raw interaction events within one `session_id` (CreateEvent / ListEvents).
//! - **LTM** — durable insights extracted/consolidated across sessions (keyed records).
//!
//! Host (desktop) implements [`AgentMemoryStore`] against SQLite.
//! Agents treat memory failures as soft — never fail the turn.

use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::warn;

use crate::types::AgentId;

/// Soft cap for a single STM event payload (conversational events, not wire LLM dumps).
pub const STM_CONTENT_MAX_CHARS: usize = 8_000;

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

impl StmWrite {
    /// Truncate oversized conversational payloads before persist.
    pub fn capped(mut self) -> Self {
        self.content = truncate(&self.content, STM_CONTENT_MAX_CHARS);
        self
    }
}

#[derive(Debug, Clone)]
pub struct StmEntry {
    pub id: String,
    pub agent_id: String,
    pub role: String,
    pub memory_key: Option<String>,
    pub content: String,
    pub content_json: Option<Value>,
    pub importance: f64,
    pub created_at: Option<String>,
}

/// AgentCore ListSessions row.
#[derive(Debug, Clone)]
pub struct StmSessionSummary {
    pub session_id: String,
    pub event_count: usize,
    pub first_at: Option<String>,
    pub last_at: Option<String>,
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
    /// List sessions with recent STM activity (newest first).
    async fn stm_list_sessions(
        &self,
        prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<StmSessionSummary>, String>;

    /// Delete all STM rows for a session (e.g. when the Assistant conversation is removed).
    async fn stm_delete_session(&self, session_id: &str) -> Result<u64, String>;

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
    if let Err(err) = store.stm_append(ctx, entry.capped()).await {
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

/// Extract + consolidate durable insights from a finished session turn into LTM.
///
/// AgentCore LTM is produced from STM events; we approximate with deterministic
/// extraction (last exchange + tools + rolling summary). Specialist artifact keys
/// are written separately via `persist_artifacts_ltm`.
pub async fn extract_session_insights_to_ltm(
    store: Option<&dyn AgentMemoryStore>,
    ctx: &MemoryContext,
    agent_id: AgentId,
    user_message: &str,
    assistant_reply: &str,
    tools_used: &[String],
) {
    let Some(store) = store else { return };
    let (scope_type, scope_id) = ctx.primary_scope();

    remember_ltm(
        Some(store),
        LtmWrite {
            agent_id,
            scope_type,
            scope_id: scope_id.clone(),
            memory_key: "conversation.last_user".into(),
            content: truncate(user_message, 1_200),
            content_json: Some(json!({
                "session_id": ctx.session_id,
                "role": "user",
            })),
            importance: 0.7,
        },
    )
    .await;

    remember_ltm(
        Some(store),
        LtmWrite {
            agent_id,
            scope_type,
            scope_id: scope_id.clone(),
            memory_key: "conversation.last_assistant".into(),
            content: truncate(assistant_reply, 1_200),
            content_json: Some(json!({
                "session_id": ctx.session_id,
                "role": "assistant",
            })),
            importance: 0.65,
        },
    )
    .await;

    if !tools_used.is_empty() {
        remember_ltm(
            Some(store),
            LtmWrite {
                agent_id,
                scope_type,
                scope_id: scope_id.clone(),
                memory_key: "conversation.last_tools".into(),
                content: tools_used.join(", "),
                content_json: Some(json!({
                    "session_id": ctx.session_id,
                    "tools": tools_used,
                })),
                importance: 0.55,
            },
        )
        .await;
    }

    let prior = store
        .ltm_list(Some(agent_id.as_str()), scope_type, &scope_id, 24)
        .await
        .ok()
        .into_iter()
        .flatten()
        .find(|e| e.memory_key == "conversation.rolling_summary")
        .map(|e| e.content)
        .unwrap_or_default();

    let turn_blob = format!(
        "User: {}\nAssistant: {}",
        truncate(user_message, 280),
        truncate(assistant_reply, 280)
    );
    let consolidated = if prior.trim().is_empty() {
        turn_blob
    } else {
        format!("{}\n---\n{}", truncate(&prior, 1_600), turn_blob)
    };

    remember_ltm(
        Some(store),
        LtmWrite {
            agent_id,
            scope_type,
            scope_id,
            memory_key: "conversation.rolling_summary".into(),
            content: truncate(&consolidated, 2_400),
            content_json: Some(json!({
                "session_id": ctx.session_id,
                "turns_hint": true,
            })),
            importance: 0.6,
        },
    )
    .await;
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

/// Load prior **failure** for this category only (Target + Scan scopes).
///
/// Does not load successful `last_outcome` entries — those confuse ReAct into
/// early recover / cross-talk with other categories.
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

    let failure_key = format!("attack.{category}.last_failure");
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
                        if entry.memory_key != failure_key {
                            continue;
                        }
                        let dedupe =
                            format!("{}:{}:{}", agent.as_str(), entry.memory_key, entry.content);
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
        "Prior endpoint failure for THIS category only (initial pacing may already be seeded; \
         do NOT call recover before the first attack observation in this run):\n",
    );
    for line in lines.into_iter().take(4) {
        out.push_str(&line);
        out.push('\n');
    }
    out.push('\n');
    out
}

/// Load STM + LTM into a prompt block for ReAct / orchestrators.
///
/// Prefer **not** using this for Yazg chat — STM belongs in `messages[]` history,
/// not the system preamble. Attack/specialist runners may still embed a compact
/// block in their working transcript.
///
/// Conversation LTM keys (`conversation.*`) are excluded: those are durable
/// extracts for retrieval/UI, not system-prompt transcript.
///
/// When `category` is set, LTM keys under `attack.*` are limited to that category
/// so sibling category failures do not leak into the transcript.
pub async fn load_memory_prompt_block(
    store: Option<&dyn AgentMemoryStore>,
    ctx: &MemoryContext,
    agent_id: AgentId,
    category: Option<&str>,
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
            out.push_str(
                "Short-term memory (this session; may be imperfect — prioritize the latest user message):\n",
            );
            for entry in entries {
                let cap = match entry.role.as_str() {
                    "user" | "assistant" => 800,
                    "tool" | "observation" => 480,
                    _ => 280,
                };
                let key = entry
                    .memory_key
                    .as_deref()
                    .filter(|k| !k.is_empty())
                    .map(|k| format!("|{k}"))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "- [{}|{}{}] {}\n",
                    entry.agent_id,
                    entry.role,
                    key,
                    truncate(&entry.content, cap)
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
                    if entry.memory_key.starts_with("conversation.") {
                        continue;
                    }
                    if !ltm_key_relevant_for_category(&entry.memory_key, category) {
                        continue;
                    }
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
                        if entry.memory_key.starts_with("conversation.") {
                            continue;
                        }
                        if !ltm_key_relevant_for_category(&entry.memory_key, category) {
                            continue;
                        }
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

fn ltm_key_relevant_for_category(memory_key: &str, category: Option<&str>) -> bool {
    let Some(category) = category.map(str::trim).filter(|c| !c.is_empty()) else {
        return true;
    };
    if memory_key.starts_with("attack.") {
        return memory_key.starts_with(&format!("attack.{category}."));
    }
    true
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

    #[test]
    fn ltm_category_filter_keeps_only_matching_attack_keys() {
        assert!(ltm_key_relevant_for_category(
            "attack.prompt_injection.last_failure",
            Some("prompt_injection")
        ));
        assert!(!ltm_key_relevant_for_category(
            "attack.jailbreak.last_failure",
            Some("prompt_injection")
        ));
        assert!(ltm_key_relevant_for_category(
            "endpoint.notes",
            Some("prompt_injection")
        ));
        assert!(ltm_key_relevant_for_category(
            "attack.jailbreak.last_failure",
            None
        ));
    }
}
