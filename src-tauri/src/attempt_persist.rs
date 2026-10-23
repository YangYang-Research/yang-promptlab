//! Persist per-agent attempt counts in SQLite `app_settings` (key `attempt`).
//!
//! Agentic scan keeps `maxAttempts` from the attack plan and does not read this row.

use promptlab_agent::{
    default_agent_attempts, parse_agent_attempts, serialize_agent_attempts, set_agent_attempts,
};
use promptlab_core::PromptLabResult;
use promptlab_storage::{AppSettingsRepository, Database, SETTING_ATTEMPT};

pub async fn hydrate_agent_attempts(db: &Database) -> PromptLabResult<()> {
    let repo = db.repositories().app_settings();
    let map = match repo.get(SETTING_ATTEMPT).await? {
        Some(row) => parse_agent_attempts(&row.value_json),
        None => default_agent_attempts(),
    };
    let raw = serialize_agent_attempts(&map);
    repo.upsert(SETTING_ATTEMPT, &raw).await?;
    set_agent_attempts(map);
    Ok(())
}
