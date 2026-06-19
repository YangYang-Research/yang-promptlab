//! Real Playwright/Chromium login recording + session replay demo (no mocks).
//!
//! Run:
//!   python3 scripts/auth-login-target.py
//!   cargo run -p aisec-auth --example record_replay
//!
//! Requires the bundled Playwright runner dependencies:
//!   (cd crates/aisec-auth/playwright && npm install && npx playwright install chromium)

use aisec_auth::{
    AuthConfig, AuthEngine, AuthEngineConfig, AuthMethod, AuthProfile, RecordLoginOptions,
    ReplayOptions, SessionStore,
};
use aisec_storage::Database;

#[tokio::main]
async fn main() {
    let base = std::env::var("AUTH_TARGET").unwrap_or_else(|_| "http://localhost:3200".into());
    let vault = std::env::temp_dir().join("aisec-auth-demo-vault");
    std::fs::create_dir_all(&vault).expect("vault dir");

    let db = Database::connect("sqlite::memory:").await.expect("db");
    let store = SessionStore::new(db, &vault).await.expect("store");
    // driver = None => real PlaywrightClient (spawns Node + Chromium). No mocks.
    let engine = AuthEngine::new(
        AuthEngineConfig::default().with_vault_dir(&vault),
        store,
        None,
    )
    .await
    .expect("engine (requires node + playwright)");

    let profile = AuthProfile {
        id: String::new(),
        project_id: None,
        name: "Demo Login".into(),
        method: AuthMethod::UsernamePassword,
        config: AuthConfig::UsernamePassword {
            login_url: format!("{base}/login"),
            username: Some("alice".into()),
            password: Some("s3cret".into()),
            password_credential_id: None,
            username_selector: "#user".into(),
            password_selector: "#pass".into(),
            submit_selector: "#submit".into(),
        },
    };
    let profile = engine.store().create_profile(&profile).await.expect("profile");

    println!("=== Record login (real Chromium via Playwright) ===");
    let (session, recording) = engine
        .record_login(&profile, RecordLoginOptions::default())
        .await
        .expect("record_login");

    println!("Session id: {}", session.id);
    println!("Recorded steps: {}", recording.steps.len());
    println!("Saved cookies ({}):", session.cookies.len());
    for c in &session.cookies {
        println!("  {}={} (domain={}, httpOnly={})", c.name, c.value, c.domain, c.http_only);
    }
    println!("Saved tokens ({}):", session.tokens.len());
    for t in &session.tokens {
        println!("  [{}] source={} value={}", t.kind, t.source, t.value);
    }
    println!("StorageState file: {:?}", session.storage_state_path);

    println!("\n=== Replay session (reuses saved storageState) ===");
    let replay = engine
        .replay_session(&session.id, &format!("{base}/dashboard"), ReplayOptions::default())
        .await
        .expect("replay_session");
    println!("Replay final_url: {}", replay.final_url);
    println!("Replay cookies: {}", replay.cookies.len());
    for c in &replay.cookies {
        println!("  {}={}", c.name, c.value);
    }

    engine.close().await.ok();
    println!("\nDone.");
}
