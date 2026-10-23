//! Per-agent attempt budget. Loaded from `app_settings` key `attempt`.
//!
//! Agentic scan does not read this map. It keeps `maxAttempts` from the attack plan.

use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::types::AgentId;

/// Lowest stored attempt count. Values below this are raised on load.
pub const MIN_AGENT_ATTEMPTS: u32 = 3;

/// `app_settings.key`.
pub const ATTEMPT_SETTING_KEY: &str = "attempt";

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
        .map(|agent| (agent.as_str().to_string(), MIN_AGENT_ATTEMPTS))
        .collect()
}

/// Replace the in-memory budget. Unknown ids are dropped. Agentic scan is dropped.
/// Counts below [`MIN_AGENT_ATTEMPTS`] are raised. Missing agents stay at the minimum.
pub fn set_agent_attempts(raw: BTreeMap<String, u32>) {
    let mut normalized = default_agent_attempts();
    for (key, value) in raw {
        if key == AgentId::AgenticAttackExecution.as_str() {
            continue;
        }
        if normalized.contains_key(&key) {
            normalized.insert(key, value.max(MIN_AGENT_ATTEMPTS));
        }
    }
    if let Ok(mut guard) = ATTEMPTS.lock() {
        *guard = Some(normalized);
    }
}

pub fn agent_attempts(agent: AgentId) -> u32 {
    let Ok(guard) = ATTEMPTS.lock() else {
        return MIN_AGENT_ATTEMPTS;
    };
    guard
        .as_ref()
        .and_then(|map| map.get(agent.as_str()).copied())
        .unwrap_or(MIN_AGENT_ATTEMPTS)
        .max(MIN_AGENT_ATTEMPTS)
}

/// JSON object stored in `app_settings.attempt`.
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
            normalized.insert(key, value.max(MIN_AGENT_ATTEMPTS));
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimum_is_three_and_agentic_is_ignored() {
        let mut raw = default_agent_attempts();
        raw.insert(AgentId::GeneratePrompt.as_str().into(), 1);
        raw.insert(AgentId::AgenticAttackExecution.as_str().into(), 9);
        let parsed = parse_agent_attempts(&serialize_agent_attempts(&raw));
        assert_eq!(parsed.get(AgentId::GeneratePrompt.as_str()), Some(&3));
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
}
