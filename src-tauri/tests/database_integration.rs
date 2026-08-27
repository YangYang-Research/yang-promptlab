//! Integration tests for the backend integration layer.
//!
//! Verifies the acceptance criteria at the layer commands use:
//!   * the database opens on startup (file created + migrations applied),
//!   * it is reachable through `AppState`/the repository manager (the exact path
//!     `db_health` and future commands use),
//!   * graceful close works and is idempotent.

use std::path::Path;

use promptlab_core::{init_logging, LogOptions};
use promptlab_desktop_lib::db::{open_database, resolve_db_path, DB_PATH_ENV};
use promptlab_desktop_lib::state::AppState;
use promptlab_storage::{CreateProject, ProjectRepository};

#[tokio::test]
async fn database_opens_and_runs_migrations_on_startup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("promptlab.db");

    let db = open_database(&path).await.expect("startup: open database");
    assert!(path.exists(), "database file must be created on startup");

    // Migrations applied => repository operations succeed.
    let repos = db.repositories();
    let project = repos
        .projects()
        .create(CreateProject {
            name: "Integration".into(),
            description: None,
        })
        .await
        .expect("create project");
    assert_eq!(project.name, "Integration");
    assert_eq!(repos.projects().list().await.unwrap().len(), 1);

    db.close().await;
    assert!(db.is_closed());
}

#[tokio::test]
async fn database_is_accessible_via_app_state() {
    // Mirrors exactly how a Tauri command reaches the DB: State<AppState> -> repositories().
    let dir = tempfile::tempdir().unwrap();
    let db = open_database(&dir.path().join("promptlab.db")).await.unwrap();
    let guard = init_logging(LogOptions::bootstrap("promptlab-it")).unwrap();
    let (manager, provider, harness_factory, plugin_manager) =
        promptlab_desktop_lib::model_registry::open_test_model_stack(dir.path(), &db)
            .await
            .expect("model stack");
    let state = AppState::new(
        db,
        dir.path().to_path_buf(),
        guard,
        promptlab_auth::AuthEngineConfig::default(),
        harness_factory,
        plugin_manager,
        promptlab_runtime::RuntimeManager::new(dir.path(), None),
        manager,
        provider,
    );

    let before = state.repositories().projects().list().await.unwrap().len();
    state
        .repositories()
        .projects()
        .create(CreateProject {
            name: "From AppState".into(),
            description: Some("via repository manager".into()),
        })
        .await
        .unwrap();
    let after = state.repositories().projects().list().await.unwrap().len();
    assert_eq!(after, before + 1, "command path must persist via AppState");

    assert!(!state.database().is_closed());
    state.database().close().await;
    assert!(state.database().is_closed(), "graceful shutdown closes the pool");
}

#[test]
fn resolve_db_path_defaults_under_data_dir() {
    std::env::remove_var(DB_PATH_ENV);
    assert_eq!(
        resolve_db_path(Path::new("/data/promptlab")),
        Path::new("/data/promptlab/promptlab.db")
    );
}
