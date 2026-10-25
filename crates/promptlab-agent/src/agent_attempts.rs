//! Per-agent attempt budget. Loaded from `app_settings` key `agent_max_attempt`.
//!
//! Agentic scan does not read this map. It keeps `maxAttempts` from the attack plan.

use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::types::AgentId;

/// Lowest stored attempt count. Values below this are raised on load.
pub const MIN_AGENT_ATTEMPTS: u32 = 1;

/// Highest stored attempt count. Values above this are lowered on load.
pub const MAX_AGENT_ATTEMPTS: u32 = 10;

/// Count written for a new agent and restored by Restore Default.
pub const DEFAULT_AGENT_ATTEMPTS: u32 = 3;

/// `app_settings.key`.
pub const ATTEMPT_SETTING_KEY: &str = "agent_max_attempt";

static ATTEMPTS: Mutex<Option<BTreeMap<String, u32>>> = Mutex::new(None);

/// Agents whose attempt count lives in the setting. Agentic scan is absent on purpose.
pub fn attempt_agent_ids() -> &'static [AgentId] {
    &[
        AgentId::Yazg,
        AgentId::AnalyzeEndpoint,
        AgentId::AttackPlan,
        AgentId::GeneratePrompt,
        AgentId::Recommend,
        AgentId::Summary,
        AgentId::JudgeCoordinator,
        AgentId::JudgeWorker,
        AgentId::ClassifierWorker,
        AgentId::AttackerWorker,
        AgentId::SequentialAttackExecution,
        AgentId::Reflection,
        AgentId::JailbreakAgent,
    ]
}

pub fn default_agent_attempts() -> BTreeMap<String, u32> {
    attempt_agent_ids()
        .iter()
        .map(|agent| (agent.as_str().to_string(), DEFAULT_AGENT_ATTEMPTS))
        .collect()
}

/// Replace the in-memory budget. Unknown ids are dropped. Agentic scan is dropped.
/// Counts outside 1..=10 are clamped. Missing agents stay at the default.
pub fn set_agent_attempts(raw: BTreeMap<String, u32>) {
    let mut normalized = default_agent_attempts();
    for (key, value) in raw {
        if key == AgentId::AgenticAttackExecution.as_str() {
            continue;
        }
        if normalized.contains_key(&key) {
            normalized.insert(key, clamp_attempt(value));
        }
    }
    if let Ok(mut guard) = ATTEMPTS.lock() {
        *guard = Some(normalized);
    }
}

/// In-memory budget, or the defaults when startup has not loaded the setting yet.
pub fn snapshot_agent_attempts() -> BTreeMap<String, u32> {
    let Ok(guard) = ATTEMPTS.lock() else {
        return default_agent_attempts();
    };
    guard.clone().unwrap_or_else(default_agent_attempts)
}

/// Next stored map after one edit. Does not write memory. Caller persists, then [`set_agent_attempts`].
pub fn prepare_agent_attempt(
    agent_id: &str,
    count: u32,
) -> Result<BTreeMap<String, u32>, &'static str> {
    if agent_id == AgentId::AgenticAttackExecution.as_str() {
        return Err("agentic scan uses maxAttempts from the attack plan");
    }
    if !(MIN_AGENT_ATTEMPTS..=MAX_AGENT_ATTEMPTS).contains(&count) {
        return Err("attempt must be from 1 to 10");
    }
    let mut map = snapshot_agent_attempts();
    if !map.contains_key(agent_id) {
        return Err("this agent does not store an attempt count");
    }
    map.insert(agent_id.to_string(), count);
    Ok(map)
}

pub fn agent_attempts(agent: AgentId) -> u32 {
    let Ok(guard) = ATTEMPTS.lock() else {
        return DEFAULT_AGENT_ATTEMPTS;
    };
    guard
        .as_ref()
        .and_then(|map| map.get(agent.as_str()).copied())
        .map(clamp_attempt)
        .unwrap_or(DEFAULT_AGENT_ATTEMPTS)
}

fn clamp_attempt(value: u32) -> u32 {
    value.clamp(MIN_AGENT_ATTEMPTS, MAX_AGENT_ATTEMPTS)
}

/// JSON object stored in `app_settings.agent_max_attempt`.
pub fn serialize_agent_attempts(map: &BTreeMap<String, u32>) -> String {
    serde_json::to_string(map).unwrap_or_else(|_| "{}".to_string())
}

pub fn parse_agent_attempts(raw: &str) -> BTreeMap<String, u32> {
    let parsed: BTreeMap<String, u32> = serde_json::from_str(raw).unwrap_or_default();
    let mut normalized = default_agent_attempts();
    for (key, value) in parsed {
        if key == AgentId::AgenticAttackExecution.as_str() {
            continue;
        }
        if normalized.contains_key(&key) {
            normalized.insert(key, clamp_attempt(value));
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_is_one_to_ten_and_agentic_is_ignored() {
        let mut raw = default_agent_attempts();
        raw.insert(AgentId::GeneratePrompt.as_str().into(), 0);
        raw.insert(AgentId::JailbreakAgent.as_str().into(), 15);
        raw.insert(AgentId::AgenticAttackExecution.as_str().into(), 9);
        let parsed = parse_agent_attempts(&serialize_agent_attempts(&raw));
        assert_eq!(parsed.get(AgentId::GeneratePrompt.as_str()), Some(&1));
        assert_eq!(parsed.get(AgentId::JailbreakAgent.as_str()), Some(&10));
        assert!(!parsed.contains_key(AgentId::AgenticAttackExecution.as_str()));
        assert_eq!(parsed.len(), attempt_agent_ids().len());
    }

    #[test]
    fn higher_count_is_kept() {
        let mut raw = default_agent_attempts();
        raw.insert("jailbreak_agent".into(), 5);
        let parsed = parse_agent_attempts(&serde_json::to_string(&raw).unwrap());
        assert_eq!(parsed.get("jailbreak_agent"), Some(&5));
    }

    #[test]
    fn prepare_rejects_low_counts_and_agentic_scan() {
        assert!(prepare_agent_attempt("jailbreak_agent", 0)
            .unwrap_err()
            .contains("1 to 10"));
        assert!(prepare_agent_attempt("jailbreak_agent", 11).is_err());
        assert!(prepare_agent_attempt(AgentId::AgenticAttackExecution.as_str(), 4)
            .unwrap_err()
            .contains("attack plan"));
        let map = prepare_agent_attempt("jailbreak_agent", 6).unwrap();
        assert_eq!(map.get("jailbreak_agent"), Some(&6));
        assert!(prepare_agent_attempt("create_project", 4).is_err());
    }
}
