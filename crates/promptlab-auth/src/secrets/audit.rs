//! Detect legacy plaintext secrets before migration.

use std::path::{Path, PathBuf};

use aisec_core::AisecResult;
use aisec_storage::{
    AuthProfileRepository, AuthSessionRepository, Database, TargetRepository,
};
use serde_json::Value;
use tracing::debug;

use super::descriptor::descriptor_has_plaintext_secrets;
use super::migrate::profile_config_has_plaintext;

/// A single legacy secret finding.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SecretAuditItem {
    pub area: String,
    pub record_id: String,
    pub field: String,
    pub message: String,
}

/// Audit summary across targets, auth profiles, sessions, and on-disk judge config.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretMigrationAudit {
    pub legacy_count: usize,
    pub targets_legacy: usize,
    pub auth_profiles_legacy: usize,
    pub sessions_legacy: usize,
    pub session_storage_legacy: usize,
    pub judge_config_legacy: usize,
    pub items: Vec<SecretAuditItem>,
}

impl SecretMigrationAudit {
    pub fn is_clean(&self) -> bool {
        self.legacy_count == 0
    }
}

/// Scan SQLite-backed records for inline secrets.
pub async fn audit_database_secrets(db: &Database, data_dir: &Path) -> AisecResult<SecretMigrationAudit> {
    let repos = db.repositories();
    let mut audit = SecretMigrationAudit::default();

    for target in repos.targets().list_all().await? {
        if descriptor_has_plaintext_secrets(&target.descriptor_json) {
            audit.targets_legacy += 1;
            audit.items.push(SecretAuditItem {
                area: "targets".into(),
                record_id: target.id.clone(),
                field: "descriptor_json".into(),
                message: "inline auth secret in target descriptor".into(),
            });
        }
    }

    for profile in repos.auth_profiles().list_all().await? {
        let config: Value =
            serde_json::from_str(&profile.config_json).unwrap_or(Value::Object(Default::default()));
        if profile_config_has_plaintext(&config) {
            audit.auth_profiles_legacy += 1;
            audit.items.push(SecretAuditItem {
                area: "auth_profiles".into(),
                record_id: profile.id.clone(),
                field: "config_json".into(),
                message: "inline password/token/key in auth profile".into(),
            });
        }
    }

    let legacy_sessions = repos
        .auth_sessions()
        .list_legacy_with_plaintext_secrets()
        .await?;
    for session in legacy_sessions {
        audit.sessions_legacy += 1;
        audit.items.push(SecretAuditItem {
            area: "auth_sessions".into(),
            record_id: session.id.clone(),
            field: "cookies_json/tokens_json".into(),
            message: "plaintext cookies or tokens in database".into(),
        });
    }

    let legacy_dir = data_dir.join("auth-vault");
    for session in repos.auth_sessions().list_all().await? {
        let Some(path_str) = session.storage_state_path else {
            continue;
        };
        let path = PathBuf::from(&path_str);
        if path.extension().and_then(|e| e.to_str()) == Some("enc") {
            continue;
        }
        let read_path = if path.is_file() {
            path
        } else {
            legacy_dir.join(format!("{}.storage.json", session.id))
        };
        if read_path.is_file() {
            audit.session_storage_legacy += 1;
            audit.items.push(SecretAuditItem {
                area: "auth_sessions".into(),
                record_id: session.id.clone(),
                field: "storage_state_path".into(),
                message: "plaintext Playwright storage artifact on disk".into(),
            });
        }
    }

    audit.legacy_count = audit.targets_legacy
        + audit.auth_profiles_legacy
        + audit.sessions_legacy
        + audit.session_storage_legacy
        + audit.judge_config_legacy;

    debug!(
        targets = audit.targets_legacy,
        profiles = audit.auth_profiles_legacy,
        sessions = audit.sessions_legacy,
        storage = audit.session_storage_legacy,
        "secret audit complete"
    );

    Ok(audit)
}

/// Merge judge-config filesystem findings into an audit report.
pub fn merge_judge_config_audit(audit: &mut SecretMigrationAudit, judge_legacy: bool) {
    if judge_legacy {
        audit.judge_config_legacy = 1;
        audit.items.push(SecretAuditItem {
            area: "judge_config".into(),
            record_id: "judge_config.json".into(),
            field: "remote.api_key".into(),
            message: "plaintext remote API key in judge config file".into(),
        });
    }
    audit.legacy_count = audit.targets_legacy
        + audit.auth_profiles_legacy
        + audit.sessions_legacy
        + audit.session_storage_legacy
        + audit.judge_config_legacy;
}

#[cfg(test)]
mod tests {
    use super::*;
    use aisec_storage::{CreateTarget, TargetRepository};

    #[tokio::test]
    async fn audit_finds_plaintext_target_descriptor() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.repositories()
            .targets()
            .create(CreateTarget {
                project_id: "p1".into(),
                name: "t".into(),
                target_type: "web".into(),
                descriptor_json: Some(serde_json::json!({
                    "url": "https://example.com",
                    "auth": { "kind": "basic", "config": { "username": "u", "password": "secret" } }
                })),
                profile_json: None,
            })
            .await
            .unwrap();

        let audit = audit_database_secrets(&db, std::path::Path::new("/tmp")).await.unwrap();
        assert_eq!(audit.targets_legacy, 1);
        assert_eq!(audit.legacy_count, 1);
    }
}
