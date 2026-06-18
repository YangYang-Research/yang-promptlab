//! Secret audit and migration IPC commands.

use aisec_auth::{
    audit_database_secrets, merge_judge_config_audit, run_database_secret_migration,
    SecretMigrationAudit, SessionStore,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::CommandResult;
use crate::judge_config::{
    audit_judge_config_legacy, migrate_judge_config_secrets, sanitize_judge_config_secrets,
};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretMigrationReport {
    pub audit_before: SecretMigrationAudit,
    pub audit_after: SecretMigrationAudit,
    pub auth_migrated: u32,
    pub targets_migrated: u32,
    pub storage_migrated: u32,
    pub judge_migrated: u32,
}

async fn full_audit(state: &AppState) -> CommandResult<SecretMigrationAudit> {
    let mut audit = audit_database_secrets(state.database(), state.data_dir())
        .await
        .map_err(crate::error::CommandError::from)?;
    let judge_legacy = audit_judge_config_legacy(state.data_dir()).await?;
    merge_judge_config_audit(&mut audit, judge_legacy);
    Ok(audit)
}

pub async fn security_audit_op(state: &AppState) -> CommandResult<SecretMigrationAudit> {
    full_audit(state).await
}

pub async fn security_migrate_secrets_op(state: &AppState) -> CommandResult<SecretMigrationReport> {
    let audit_before = full_audit(state).await?;

    let vault_dir = aisec_auth::auth_sessions_dir(state.data_dir());
    let store = SessionStore::new(state.database().clone(), vault_dir)
        .await
        .map_err(crate::error::CommandError::from)?;

    let db_result = run_database_secret_migration(
        state.database(),
        state.data_dir(),
        store.secrets(),
        store.encrypted_vault(),
    )
    .await
    .map_err(crate::error::CommandError::from)?;

    let judge_migrated = migrate_judge_config_secrets(state.data_dir(), store.secrets()).await?;

    let audit_after = full_audit(state).await?;

    Ok(SecretMigrationReport {
        audit_before,
        audit_after,
        auth_migrated: db_result.auth_migrated,
        targets_migrated: db_result.targets_migrated,
        storage_migrated: db_result.storage_migrated,
        judge_migrated,
    })
}

#[tauri::command]
pub async fn security_audit(state: State<'_, AppState>) -> CommandResult<SecretMigrationAudit> {
    security_audit_op(&state).await
}

#[tauri::command]
pub async fn security_migrate_secrets(
    state: State<'_, AppState>,
) -> CommandResult<SecretMigrationReport> {
    security_migrate_secrets_op(&state).await
}

/// Sanitize judge config before persisting (used by judge save command).
pub fn sanitize_judge_on_save(config: &mut aisec_judge::JudgeProviderConfig) -> CommandResult<()> {
    let secrets = aisec_auth::SecretStore::new().map_err(crate::error::CommandError::from)?;
    sanitize_judge_config_secrets(config, &secrets)?;
    Ok(())
}
