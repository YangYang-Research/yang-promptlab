//! Persist [`EnvironmentConfig`] in SQLite `app_settings` (key `environment`).

use promptlab_core::{
    ensure_environment, resolve_paths, EnvironmentConfig, EnvironmentPaths, PromptLabResult,
};
use promptlab_storage::{AppSettingsRepository, Database, SETTING_ENVIRONMENT};

pub async fn load_environment_config(db: &Database) -> PromptLabResult<Option<EnvironmentConfig>> {
    let record = db
        .repositories()
        .app_settings()
        .get(SETTING_ENVIRONMENT)
        .await?;
    let Some(row) = record else {
        return Ok(None);
    };
    let config = serde_json::from_str(&row.value_json).map_err(|err| {
        promptlab_core::PromptLabError::config(format!(
            "invalid environment settings in database: {err}"
        ))
    })?;
    Ok(Some(config))
}

pub async fn save_environment_config(
    db: &Database,
    config: &EnvironmentConfig,
) -> PromptLabResult<()> {
    let raw = serde_json::to_string(config).map_err(|err| {
        promptlab_core::PromptLabError::internal(format!(
            "serialize environment settings: {err}"
        ))
    })?;
    db.repositories()
        .app_settings()
        .upsert(SETTING_ENVIRONMENT, &raw)
        .await?;
    Ok(())
}

/// After the DB is open at the bootstrap path, load path overrides from SQLite.
///
/// `root` / `workspaces` stay on the bootstrap values so the already-open database
/// remains authoritative (`PROMPTLAB_ROOT` / default `~/.promptlab`).
pub async fn hydrate_environment_paths(
    db: &Database,
    bootstrap: &EnvironmentPaths,
) -> PromptLabResult<EnvironmentPaths> {
    let mut config = match load_environment_config(db).await? {
        Some(config) => config,
        None => {
            let config = EnvironmentConfig {
                root: Some(bootstrap.root.clone()),
                ..Default::default()
            };
            save_environment_config(db, &config).await?;
            config
        }
    };

    config.root = Some(bootstrap.root.clone());
    config.workspaces = None;

    let mut paths = resolve_paths(&config);
    paths.root = bootstrap.root.clone();
    paths.workspaces = bootstrap.workspaces.clone();
    paths.config = bootstrap.config.clone();

    ensure_environment(&paths)?;
    Ok(paths)
}
