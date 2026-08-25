//! Lifetime AI Runtime token usage by agent / sub-agent.
//!
//! Hot-path recording stays in-memory and queues dirty state for JSON persistence.
//! Call sites set [`CURRENT_AGENT`] via [`with_agent`] before completions so
//! leaf adapters ([`crate::provider::RemoteProviderAdapter`] /
//! [`crate::provider::RemoteProviderAdapter`]) attribute tokens correctly.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

tokio::task_local! {
    static CURRENT_AGENT: String;
}

const UNKNOWN_AGENT: &str = "unknown";
/// Bucket for AI Runtime completions that are not tied to a Yazg agent
/// (health checks, connectivity probes, and other unlabeled host calls).
pub const RUNTIME_SYSTEM_AGENT: &str = "runtime_system";

const RUNTIME_SYSTEM_NOTE: &str =
    "Health checks, connectivity probes, and unlabeled runtime calls.";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentTokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub calls: u64,
}

impl AgentTokenUsage {
    fn add(&mut self, input: u64, output: u64) {
        self.input_tokens = self.input_tokens.saturating_add(input);
        self.output_tokens = self.output_tokens.saturating_add(output);
        self.calls = self.calls.saturating_add(1);
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsageSnapshot {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_calls: u64,
    pub agents: Vec<AgentTokenUsageRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentTokenUsageRow {
    pub agent_id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub calls: u64,
}

#[derive(Debug, Default)]
struct TokenUsageState {
    by_agent: BTreeMap<String, AgentTokenUsage>,
    dirty: bool,
}

fn monitor() -> &'static Mutex<TokenUsageState> {
    static MONITOR: OnceLock<Mutex<TokenUsageState>> = OnceLock::new();
    MONITOR.get_or_init(|| Mutex::new(TokenUsageState::default()))
}

/// Run an async block with an agent attribution label for nested completions.
pub async fn with_agent<F, Fut, T>(agent_id: &str, f: F) -> T
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    let trimmed = agent_id.trim();
    let label = if trimmed.is_empty() || trimmed == UNKNOWN_AGENT {
        RUNTIME_SYSTEM_AGENT.to_string()
    } else {
        trimmed.to_string()
    };
    CURRENT_AGENT.scope(label, f()).await
}

fn current_agent_id() -> Option<String> {
    CURRENT_AGENT.try_with(|value| value.clone()).ok()
}

fn resolve_record_agent_id() -> String {
    match current_agent_id() {
        Some(agent) if !agent.trim().is_empty() && agent != UNKNOWN_AGENT => agent,
        _ => RUNTIME_SYSTEM_AGENT.to_string(),
    }
}

/// Rough token estimate when providers omit usage (≈ 4 chars / token).
pub fn estimate_tokens(text: &str) -> u64 {
    let len = text.len() as u64;
    if len == 0 {
        0
    } else {
        (len + 3) / 4
    }
}

/// Record one completion's token usage under the current agent label.
///
/// Calls without an agent scope are counted under [`RUNTIME_SYSTEM_AGENT`]
/// (health / connectivity / unlabeled runtime work).
pub fn record_completion(input_tokens: u64, output_tokens: u64) {
    let agent = resolve_record_agent_id();
    let Ok(mut state) = monitor().lock() else {
        return;
    };
    state
        .by_agent
        .entry(agent)
        .or_default()
        .add(input_tokens, output_tokens);
    state.dirty = true;
}

/// Replace in-memory counters (hydrate from durable storage).
pub fn replace_all(by_agent: BTreeMap<String, AgentTokenUsage>) {
    let Ok(mut state) = monitor().lock() else {
        return;
    };
    state.by_agent = migrate_legacy_buckets(by_agent);
    state.dirty = false;
}

fn migrate_legacy_buckets(
    mut by_agent: BTreeMap<String, AgentTokenUsage>,
) -> BTreeMap<String, AgentTokenUsage> {
    if let Some(legacy) = by_agent.remove(UNKNOWN_AGENT) {
        let entry = by_agent.entry(RUNTIME_SYSTEM_AGENT.to_string()).or_default();
        entry.input_tokens = entry.input_tokens.saturating_add(legacy.input_tokens);
        entry.output_tokens = entry.output_tokens.saturating_add(legacy.output_tokens);
        entry.calls = entry.calls.saturating_add(legacy.calls);
    }
    by_agent
}

/// Clear all counters and mark dirty so persistence can wipe the file.
pub fn reset() {
    let Ok(mut state) = monitor().lock() else {
        return;
    };
    state.by_agent.clear();
    state.dirty = true;
}

/// Merge legacy `unknown` counters into [`RUNTIME_SYSTEM_AGENT`] and mark dirty.
pub fn migrate_unattributed() {
    let Ok(mut state) = monitor().lock() else {
        return;
    };
    if let Some(legacy) = state.by_agent.remove(UNKNOWN_AGENT) {
        let entry = state
            .by_agent
            .entry(RUNTIME_SYSTEM_AGENT.to_string())
            .or_default();
        entry.input_tokens = entry.input_tokens.saturating_add(legacy.input_tokens);
        entry.output_tokens = entry.output_tokens.saturating_add(legacy.output_tokens);
        entry.calls = entry.calls.saturating_add(legacy.calls);
        state.dirty = true;
    }
}

/// Take a durable snapshot if dirty; clears the dirty flag when returning Some.
pub fn take_dirty_snapshot() -> Option<BTreeMap<String, AgentTokenUsage>> {
    let Ok(mut state) = monitor().lock() else {
        return None;
    };
    if !state.dirty {
        return None;
    }
    state.dirty = false;
    Some(state.by_agent.clone())
}

/// Force a clone of current counters (for IPC / tests).
pub fn export_map() -> BTreeMap<String, AgentTokenUsage> {
    let Ok(state) = monitor().lock() else {
        return BTreeMap::new();
    };
    state.by_agent.clone()
}

pub fn humanize_agent_id(agent_id: &str) -> String {
    match agent_id {
        "yazg" => "Yazg".into(),
        "analyze_endpoint" => "AnalyzeEndpointAgent".into(),
        "attack_plan" => "AttackPlanAgent".into(),
        "generate_prompt" => "GeneratePromptAgent".into(),
        "recommend" => "RecommendAgent".into(),
        "summary" => "SummaryAgent".into(),
        "judge_coordinator" => "JudgeCoordinatorAgent".into(),
        "judge_worker" => "JudgeWorker".into(),
        "classifier_worker" => "ClassifierWorker".into(),
        "attacker_worker" => "AttackerWorker".into(),
        "agentic_attack_execution" => "AgenticAttackExecutionAgent".into(),
        "sequential_attack_execution" => "SequentialAttackExecutionAgent".into(),
        "reflection" => "ReflectionAgent".into(),
        "create_project" => "CreateProjectTool".into(),
        RUNTIME_SYSTEM_AGENT | "unknown" => "Runtime system".into(),
        other => other
            .split('_')
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn agent_note(agent_id: &str) -> Option<String> {
    match agent_id {
        RUNTIME_SYSTEM_AGENT => Some(RUNTIME_SYSTEM_NOTE.to_string()),
        "judge_coordinator" => Some(
            "ReAct turns that dispatch JudgeWorker / ClassifierWorker / AttackerWorker tools."
                .into(),
        ),
        "sequential_attack_execution" => Some(
            "ReAct generate / attack / recover picks during sequential scans."
                .into(),
        ),
        "agentic_attack_execution" => Some(
            "ReAct generate / attack / recover / reflect / adapt picks during agentic scans. ReflectionAgent votes are counted separately.".into(),
        ),
        "reflection" => Some(
            "Counts when an agentic scan has reflection enabled."
                .into(),
        ),
        "summary" => Some(
            "Counts when you generate a project or scan summary (Project details or Yazg)."
                .into(),
        ),
        _ => None,
    }
}

/// Canonical agent / sub-agent ids shown in Settings → Usage.
pub fn known_agent_ids() -> &'static [&'static str] {
    &[
        "yazg",
        "analyze_endpoint",
        "attack_plan",
        "generate_prompt",
        "recommend",
        "summary",
        "judge_coordinator",
        "judge_worker",
        "classifier_worker",
        "attacker_worker",
        "agentic_attack_execution",
        "sequential_attack_execution",
        "reflection",
        RUNTIME_SYSTEM_AGENT,
    ]
}

pub fn snapshot() -> TokenUsageSnapshot {
    let map = export_map();
    let mut total_input = 0u64;
    let mut total_output = 0u64;
    let mut total_calls = 0u64;

    for usage in map.values() {
        total_input = total_input.saturating_add(usage.input_tokens);
        total_output = total_output.saturating_add(usage.output_tokens);
        total_calls = total_calls.saturating_add(usage.calls);
    }

    let agents: Vec<AgentTokenUsageRow> = known_agent_ids()
        .iter()
        .map(|agent_id| {
            let usage = map.get(*agent_id).cloned().unwrap_or_default();
            AgentTokenUsageRow {
                label: humanize_agent_id(agent_id),
                note: agent_note(agent_id),
                agent_id: (*agent_id).to_string(),
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                calls: usage.calls,
            }
        })
        .collect();

    TokenUsageSnapshot {
        total_input_tokens: total_input,
        total_output_tokens: total_output,
        total_calls,
        agents,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn records_under_agent_label() {
        reset();
        with_agent("recommend", || async {
            record_completion(10, 20);
            record_completion(5, 5);
        })
        .await;
        let snap = snapshot();
        assert_eq!(snap.total_input_tokens, 15);
        assert_eq!(snap.total_output_tokens, 25);
        assert_eq!(snap.total_calls, 2);
        assert!(snap.agents.len() >= known_agent_ids().len());
        let recommend = snap
            .agents
            .iter()
            .find(|row| row.agent_id == "recommend")
            .expect("recommend row");
        assert_eq!(recommend.input_tokens, 15);
        assert_eq!(recommend.output_tokens, 25);
        assert_eq!(recommend.calls, 2);
        let unused = snap
            .agents
            .iter()
            .find(|row| row.agent_id == "summary")
            .expect("summary row");
        assert_eq!(unused.calls, 0);
        reset();
    }

    #[tokio::test]
    async fn records_runtime_system_when_unscoped() {
        reset();
        record_completion(99, 11);
        let snap = snapshot();
        assert_eq!(snap.total_calls, 1);
        let system = snap
            .agents
            .iter()
            .find(|row| row.agent_id == RUNTIME_SYSTEM_AGENT)
            .expect("runtime_system row");
        assert_eq!(system.input_tokens, 99);
        assert_eq!(system.output_tokens, 11);
        assert_eq!(system.label, "Runtime system");
        assert!(system.note.as_deref().unwrap_or("").contains("Health checks"));
        assert!(!snap.agents.iter().any(|row| row.agent_id == "unknown"));
        let sequential = snap
            .agents
            .iter()
            .find(|row| row.agent_id == "sequential_attack_execution")
            .expect("sequential row");
        assert!(sequential.note.as_deref().unwrap_or("").contains("ReAct"));
        reset();
    }

    #[test]
    fn estimate_tokens_is_roughly_chars_over_four() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcdefgh"), 2);
    }
}
