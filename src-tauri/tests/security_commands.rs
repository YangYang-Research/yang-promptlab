//! Security migration IPC integration tests.

use std::path::Path;

use aisec_auth::descriptor_has_plaintext_secrets;
use aisec_core::{init_logging, LogOptions};
use aisec_desktop_lib::commands::security::{security_audit_op, security_migrate_secrets_op};
use aisec_desktop_lib::db::open_database;
use aisec_desktop_lib::judge_config::{judge_config_path, load_judge_config};
use aisec_desktop_lib::state::AppState;
use aisec_judge::JudgeProviderConfig;
use aisec_storage::{
    AuthProfileRepository, AuthSessionRepository, CreateAuthProfile, CreateAuthSessionRecord,
    CreateProject, CreateTarget, ProjectRepository, TargetRepository,
};

async fn make_state(dir: &Path) -> AppState {
    let db = open_database(&dir.join("aisec.db")).await.expect("open db");
    let guard = init_logging(LogOptions::bootstrap("security-it")).unwrap();
    let (manager, provider, meta, harness_factory, plugin_manager) =
        aisec_desktop_lib::model_registry::open_test_model_stack(dir).expect("model stack");
    AppState::new(
        db,
        dir.to_path_buf(),
        guard,
        aisec_auth::AuthEngineConfig::default(),
        harness_factory,
        plugin_manager,
        aisec_runtime::RuntimeManager::new(dir, None),
        manager,
        provider,
        meta,
    )
}

#[tokio::test]
async fn audit_detects_legacy_target_session_and_judge_config() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(dir.path()).await;

    let project = state
        .repositories()
        .projects()
        .create(CreateProject {
            name: "p".into(),
            description: None,
        })
        .await
        .unwrap();

    state
        .repositories()
        .targets()
        .create(CreateTarget {
            project_id: project.id,
            name: "t".into(),
            target_type: "web".into(),
            descriptor_json: Some(serde_json::json!({
                "url": "https://example.com",
                "auth": { "kind": "basic", "config": { "username": "u", "password": "secret" } }
            })),
        })
        .await
        .unwrap();

    let profile = state
        .repositories()
        .auth_profiles()
        .create(CreateAuthProfile {
            project_id: None,
            name: "profile".into(),
            method: "jwt".into(),
            config_json: serde_json::json!({ "token": "legacy-token" }),
        })
        .await
        .unwrap();

    state
        .repositories()
        .auth_sessions()
        .create(CreateAuthSessionRecord {
            profile_id: profile.id,
            status: None,
            cookies_json: Some(serde_json::json!([{"name":"sid","value":"cookie-secret"}])),
            tokens_json: None,
            credential_reference_id: None,
            storage_state_path: None,
            expires_at: None,
            validation_status: None,
            user_identity: None,
        })
        .await
        .unwrap();

    let mut judge = JudgeProviderConfig::default();
    judge.remote.api_key = "sk-test-key".into();
    let judge_json = serde_json::to_string_pretty(&judge).unwrap();
    tokio::fs::write(judge_config_path(state.data_dir()), judge_json)
        .await
        .unwrap();

    let audit = security_audit_op(&state).await.expect("audit");
    assert!(audit.targets_legacy >= 1);
    assert!(audit.auth_profiles_legacy >= 1);
    assert!(audit.sessions_legacy >= 1);
    assert_eq!(audit.judge_config_legacy, 1);
    assert!(audit.legacy_count >= 4);
}

#[tokio::test]
async fn migrate_clears_plaintext_secrets() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(dir.path()).await;

    let project = state
        .repositories()
        .projects()
        .create(CreateProject {
            name: "p".into(),
            description: None,
        })
        .await
        .unwrap();

    let descriptor = serde_json::json!({
        "url": "https://example.com",
        "auth": { "kind": "basic", "config": { "username": "u", "password": "migrate-me" } }
    });
    let target = state
        .repositories()
        .targets()
        .create(CreateTarget {
            project_id: project.id,
            name: "t".into(),
            target_type: "web".into(),
            descriptor_json: Some(descriptor.clone()),
        })
        .await
        .unwrap();

    let mut judge = JudgeProviderConfig::default();
    judge.remote.api_key = "sk-migrate".into();
    tokio::fs::write(
        judge_config_path(state.data_dir()),
        serde_json::to_string_pretty(&judge).unwrap(),
    )
    .await
    .unwrap();

    let report = security_migrate_secrets_op(&state)
        .await
        .expect("migrate");
    assert!(report.audit_before.legacy_count >= 2);
    assert_eq!(report.audit_after.legacy_count, 0);
    assert!(report.targets_migrated >= 1);
    assert!(report.judge_migrated >= 1);

    let updated = state.repositories().targets().get(&target.id).await.unwrap();
    assert!(
        !descriptor_has_plaintext_secrets(&updated.descriptor_json),
        "target descriptor must not contain plaintext password"
    );

    let judge_raw = tokio::fs::read_to_string(judge_config_path(state.data_dir()))
        .await
        .unwrap();
    assert!(
        !judge_raw.contains("sk-migrate"),
        "judge config file must not contain plaintext api key"
    );

    let loaded = load_judge_config(state.data_dir()).await.unwrap();
    assert!(loaded.remote.api_key.is_empty());
    assert!(loaded.remote.api_key_credential_id.is_some());
}
