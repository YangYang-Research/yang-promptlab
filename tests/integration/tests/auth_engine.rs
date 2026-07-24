use promptlab_auth::{
    AuthConfig, AuthEngine, AuthEngineConfig, AuthMethod, AuthProfile, RecordLoginOptions,
    MockPlaywrightDriver, SessionStore,
};
use promptlab_storage::Database;
use std::sync::Arc;

#[tokio::test]
async fn auth_engine_persists_session_to_database() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::connect("sqlite::memory:").await.expect("db");
    let store = SessionStore::new(db.clone(), dir.path())
        .await
        .expect("store");

    let driver = Arc::new(MockPlaywrightDriver::login_success());
    let engine = AuthEngine::new(
        AuthEngineConfig::default().with_vault_dir(dir.path()),
        store,
        Some(driver),
    )
    .await
    .expect("engine");

    let profile_id = uuid::Uuid::now_v7().to_string();
    let profile = AuthProfile {
        id: profile_id.clone(),
        project_id: None,
        name: "Integration".into(),
        method: AuthMethod::UsernamePassword,
        config: AuthConfig::UsernamePassword {
            login_url: "https://example.com/login".into(),
            username: Some("u".into()),
            password: Some("p".into()),
            username_selector: "#u".into(),
            password_selector: "#p".into(),
            submit_selector: "#s".into(),
        },
    };

    let (session, _) = engine
        .record_login(&profile, RecordLoginOptions::default())
        .await
        .expect("record");

    let loaded = db
        .repositories()
        .auth_sessions()
        .get(&session.id)
        .await
        .expect("session row");

    assert_eq!(loaded.profile_id, profile_id);
    assert!(loaded.cookies_json.is_some());
}
