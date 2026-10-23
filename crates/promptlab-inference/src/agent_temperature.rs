//! Per-agent sampling temperature.
//!
//! Each agent that calls an LLM has a preset. A stored value replaces that preset.
//! An id with neither falls back to the model temperature.

use std::collections::BTreeMap;
use std::sync::Mutex;

static TEMPERATURES: Mutex<BTreeMap<String, f32>> = Mutex::new(BTreeMap::new());

/// Preset sampling temperature for every agent that calls an LLM.
/// Cold for decisions, warmer for prose and attack prompts.
pub fn default_agent_temperatures() -> BTreeMap<String, f32> {
    BTreeMap::from([
        ("yazg".into(), 0.2),
        ("analyze_endpoint_agent".into(), 0.0),
        ("attack_plan_agent".into(), 0.2),
        ("generate_prompt_agent".into(), 0.7),
        ("recommend_agent".into(), 0.4),
        ("summary_agent".into(), 0.4),
        ("judge_coordinator_agent".into(), 0.1),
        ("judge_worker".into(), 0.0),
        ("classifier_worker".into(), 0.0),
        ("attacker_worker".into(), 0.1),
        ("agentic_attack_execution_agent".into(), 0.1),
        ("sequential_attack_execution_agent".into(), 0.1),
        ("reflection_agent".into(), 0.0),
        ("jailbreak_agent".into(), 0.8),
    ])
}

/// Presets first. A stored value for a known agent replaces its preset.
pub fn merge_agent_temperatures(stored: BTreeMap<String, f32>) -> BTreeMap<String, f32> {
    let mut map = default_agent_temperatures();
    for (key, value) in stored {
        if value.is_finite() && (0.0..=2.0).contains(&value) && map.contains_key(&key) {
            map.insert(key, value);
        }
    }
    map
}

pub fn set_agent_temperatures(map: BTreeMap<String, f32>) {
    if let Ok(mut guard) = TEMPERATURES.lock() {
        *guard = map;
    }
}

pub fn snapshot_agent_temperatures() -> BTreeMap<String, f32> {
    TEMPERATURES
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default()
}

pub fn agent_temperature(agent_id: &str) -> Option<f32> {
    TEMPERATURES
        .lock()
        .ok()
        .and_then(|guard| guard.get(agent_id).copied())
        .filter(|value| value.is_finite())
}

/// `None` restores the preset. An unknown id is removed.
pub fn prepare_agent_temperature(
    agent_id: &str,
    temperature: Option<f32>,
) -> Result<BTreeMap<String, f32>, &'static str> {
    let agent_id = agent_id.trim();
    if agent_id.is_empty() {
        return Err("agent id is required");
    }
    let mut map = snapshot_agent_temperatures();
    match temperature {
        None => match default_agent_temperatures().get(agent_id).copied() {
            Some(preset) => {
                map.insert(agent_id.to_string(), preset);
            }
            None => {
                map.remove(agent_id);
            }
        },
        Some(value) if value.is_finite() && (0.0..=2.0).contains(&value) => {
            map.insert(agent_id.to_string(), value);
        }
        Some(_) => return Err("temperature must be between 0 and 2"),
    }
    Ok(map)
}

/// Agent override, then the request temperature, then the client default.
/// Health probes keep the request temperature.
pub fn resolve_call_temperature(
    agent_id: Option<&str>,
    purpose: Option<&str>,
    request: Option<f32>,
    fallback: f32,
) -> f32 {
    if purpose.unwrap_or("").trim() == "health" {
        return request.unwrap_or(0.0);
    }
    agent_id
        .and_then(agent_temperature)
        .or(request)
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_wins_and_missing_falls_back() {
        set_agent_temperatures(BTreeMap::from([("jailbreak_agent".into(), 0.9)]));
        assert_eq!(
            resolve_call_temperature(Some("jailbreak_agent"), Some("jailbreak_agent"), Some(0.7), 0.1),
            0.9
        );
        assert_eq!(
            resolve_call_temperature(Some("yazg"), Some("yazg"), Some(0.7), 0.1),
            0.7
        );
        assert_eq!(
            resolve_call_temperature(None, Some("health"), Some(0.0), 0.7),
            0.0
        );
        set_agent_temperatures(BTreeMap::new());
    }

    #[test]
    fn prepare_rejects_out_of_range_and_clear_removes() {
        set_agent_temperatures(BTreeMap::new());
        assert!(prepare_agent_temperature("yazg", Some(2.5)).is_err());
        let map = prepare_agent_temperature("yazg", Some(0.4)).unwrap();
        set_agent_temperatures(map);
        assert_eq!(agent_temperature("yazg"), Some(0.4));
        let cleared = prepare_agent_temperature("yazg", None).unwrap();
        set_agent_temperatures(cleared);
        assert_eq!(agent_temperature("yazg"), Some(0.2));
    }

    #[test]
    fn merge_keeps_override_and_fills_presets() {
        let merged = merge_agent_temperatures(BTreeMap::from([
            ("jailbreak_agent".into(), 0.9),
            ("not_an_agent".into(), 1.0),
        ]));
        assert_eq!(merged.get("jailbreak_agent"), Some(&0.9));
        assert_eq!(merged.get("generate_prompt_agent"), Some(&0.7));
        assert_eq!(merged.get("judge_worker"), Some(&0.0));
        assert!(!merged.contains_key("not_an_agent"));
        assert_eq!(merged.len(), default_agent_temperatures().len());
    }
}
