//! Persist per-agent attempt counts and sampling temperatures in `app_settings`.
//!
//! Agentic scan keeps `maxAttempts` from the attack plan and does not read
//! `agent_max_attempt`. `agent_temperature` stores one value per agent. A missing
//! entry is filled from the preset. An unknown id still falls back to the model.

use std::collections::BTreeMap;

use promptlab_agent::{
    default_agent_attempts, parse_agent_attempts, prepare_agent_attempt, serialize_agent_attempts,
    set_agent_attempts,
};
use promptlab_core::PromptLabResult;
use promptlab_inference::{
    merge_agent_temperatures, prepare_agent_temperature, set_agent_temperatures,
    snapshot_agent_temperatures,
};
use promptlab_storage::{
    AppSettingsRepository, Database, SETTING_AGENT_MAX_ATTEMPT, SETTING_AGENT_TEMPERATURE,
    SETTING_ATTEMPT_LEGACY,
};

pub async fn hydrate_agent_attempts(db: &Database) -> PromptLabResult<()> {
    let repo = db.repositories().app_settings();
    let map = if let Some(row) = repo.get(SETTING_AGENT_MAX_ATTEMPT).await? {
        parse_agent_attempts(&row.value_json)
    } else if let Some(legacy) = repo.get(SETTING_ATTEMPT_LEGACY).await? {
        let map = parse_agent_attempts(&legacy.value_json);
        repo.delete(SETTING_ATTEMPT_LEGACY).await?;
        map
    } else {
        default_agent_attempts()
    };
    let raw = serialize_agent_attempts(&map);
    repo.upsert(SETTING_AGENT_MAX_ATTEMPT, &raw).await?;
    set_agent_attempts(map);
    Ok(())
}

pub async fn hydrate_agent_temperatures(db: &Database) -> PromptLabResult<()> {
    let repo = db.repositories().app_settings();
    let stored = match repo.get(SETTING_AGENT_TEMPERATURE).await? {
        Some(row) => parse_temperature_map(&row.value_json),
        None => BTreeMap::new(),
    };
    let map = merge_agent_temperatures(stored);
    let raw = serialize_temperature_map(&map);
    repo.upsert(SETTING_AGENT_TEMPERATURE, &raw).await?;
    set_agent_temperatures(map);
    Ok(())
}

/// Validate one attempt, write the full map, then update memory.
pub async fn persist_agent_attempt(
    db: &Database,
    agent_id: &str,
    count: u32,
) -> PromptLabResult<BTreeMap<String, u32>> {
    let map = prepare_agent_attempt(agent_id, count)
        .map_err(promptlab_core::PromptLabError::invalid_input)?;
    let raw = serialize_agent_attempts(&map);
    db.repositories()
        .app_settings()
        .upsert(SETTING_AGENT_MAX_ATTEMPT, &raw)
        .await?;
    set_agent_attempts(map.clone());
    Ok(map)
}

/// `None` restores that agent's preset.
pub async fn persist_agent_temperature(
    db: &Database,
    agent_id: &str,
    temperature: Option<f32>,
) -> PromptLabResult<Option<f32>> {
    let map = prepare_agent_temperature(agent_id, temperature)
        .map_err(promptlab_core::PromptLabError::invalid_input)?;
    let raw = serialize_temperature_map(&map);
    db.repositories()
        .app_settings()
        .upsert(SETTING_AGENT_TEMPERATURE, &raw)
        .await?;
    set_agent_temperatures(map);
    Ok(snapshot_agent_temperatures().get(agent_id).copied())
}

fn serialize_temperature_map(map: &BTreeMap<String, f32>) -> String {
    serde_json::to_string(map).unwrap_or_else(|_| "{}".to_string())
}

fn parse_temperature_map(raw: &str) -> BTreeMap<String, f32> {
    let parsed: BTreeMap<String, serde_json::Value> = serde_json::from_str(raw).unwrap_or_default();
    parsed
        .into_iter()
        .filter_map(|(key, value)| {
            let temperature = value.as_f64()? as f32;
            if key.trim().is_empty() || !temperature.is_finite() || !(0.0..=2.0).contains(&temperature)
            {
                return None;
            }
            Some((key, temperature))
        })
        .collect()
}