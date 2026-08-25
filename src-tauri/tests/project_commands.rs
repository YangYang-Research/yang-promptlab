//! Integration tests for project IPC commands:
//! `project_create`, `project_list`, `project_get`, `project_delete`.
//!
//! Exercises the `*_op` functions (same logic as the `#[tauri::command]` wrappers)
//! against a real file-backed SQLite database — no mocks.

use std::path::Path;

use promptlab_core::{init_logging, LogOptions};
use promptlab_desktop_lib::commands::projects::{
    project_create_op, project_delete_op, project_get_op, project_list_op,
};
use promptlab_desktop_lib::db::open_database;
use promptlab_desktop_lib::state::AppState;

async fn make_state(dir: &Path) -> AppState {
    let db = open_database(&dir.join("promptlab.db")).await.expect("open db");
    let guard = init_logging(LogOptions::bootstrap("project-it")).unwrap();
    let (manager, provider, harness_factory, plugin_manager) =
        promptlab_desktop_lib::model_registry::open_test_model_stack(dir, &db).await.expect("model stack");
    AppState::new(
        db,
        dir.to_path_buf(),
        guard,
        promptlab_auth::AuthEngineConfig::default(),
        harness_factory,
        plugin_manager,
        promptlab_runtime::RuntimeManager::new(dir, None),
        manager,
        provider,
    )
}

async fn sqlite_project_count(state: &AppState) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM projects")
        .fetch_one(state.database().pool())
        .await
        .expect("count projects in SQLite")
}

// ---------------------------------------------------------------------------
// project_create
// ---------------------------------------------------------------------------

#[tokio::test]
async fn project_create_persists_and_get_returns_it() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(dir.path()).await;

    assert_eq!(sqlite_project_count(&state).await, 0);

    let created = project_create_op(&state, "Acme Pentest".into(), Some("Q2 review".into()))
        .await
        .expect("create");
    assert_eq!(created.name, "Acme Pentest");
    assert_eq!(created.description.as_deref(), Some("Q2 review"));
    assert!(!created.id.is_empty());
    assert_eq!(sqlite_project_count(&state).await, 1);

    let got = project_get_op(&state, created.id.clone()).await.expect("get");
    assert_eq!(got.id, created.id);
    assert_eq!(got.name, "Acme Pentest");
}

#[tokio::test]
async fn project_create_rejects_empty_name() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(dir.path()).await;

    let err = project_create_op(&state, "   ".into(), None)
        .await
        .expect_err("empty name should error");
    assert_eq!(err.code, "INVALID_INPUT");
    assert_eq!(sqlite_project_count(&state).await, 0);
}

// ---------------------------------------------------------------------------
// project_list
// ---------------------------------------------------------------------------

#[tokio::test]
async fn project_list_returns_all_created() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(dir.path()).await;

    assert!(project_list_op(&state).await.unwrap().is_empty());

    project_create_op(&state, "Alpha".into(), None).await.unwrap();
    project_create_op(&state, "Beta".into(), None).await.unwrap();

    assert_eq!(sqlite_project_count(&state).await, 2);

    let all = project_list_op(&state).await.expect("list");
    assert_eq!(all.len(), 2);
    let names: Vec<&str> = all.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"Alpha"));
    assert!(names.contains(&"Beta"));
}

// ---------------------------------------------------------------------------
// project_get
// ---------------------------------------------------------------------------

#[tokio::test]
async fn project_get_missing_returns_typed_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(dir.path()).await;

    let err = project_get_op(&state, "does-not-exist".into())
        .await
        .expect_err("should error");
    assert_eq!(err.code, "NOT_FOUND");
}

// ---------------------------------------------------------------------------
// project_delete
// ---------------------------------------------------------------------------

#[tokio::test]
async fn project_delete_removes_from_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(dir.path()).await;

    let project = project_create_op(&state, "Temporary".into(), None).await.unwrap();
    assert_eq!(sqlite_project_count(&state).await, 1);

    project_delete_op(&state, project.id.clone()).await.expect("delete");

    assert_eq!(sqlite_project_count(&state).await, 0);
    assert!(project_get_op(&state, project.id.clone()).await.is_err());
    assert!(project_list_op(&state).await.unwrap().is_empty());

    let err = project_delete_op(&state, project.id).await.expect_err("re-delete errors");
    assert_eq!(err.code, "NOT_FOUND");
}

// ---------------------------------------------------------------------------
// Acceptance: project persists in SQLite (survives pool drop + reopen)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn project_persists_in_sqlite_after_database_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("promptlab.db");

    let project_id = {
        let state = make_state(dir.path()).await;
        let created = project_create_op(&state, "Persisted".into(), Some("on disk".into()))
            .await
            .expect("create");
        assert_eq!(sqlite_project_count(&state).await, 1);
        created.id
    };

    assert!(db_path.exists(), "SQLite file must exist on disk");

    let reopened = make_state(dir.path()).await;
    assert_eq!(sqlite_project_count(&reopened).await, 1);

    let listed = project_list_op(&reopened).await.expect("list after reopen");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, project_id);

    let got = project_get_op(&reopened, project_id).await.expect("get after reopen");
    assert_eq!(got.name, "Persisted");
    assert_eq!(got.description.as_deref(), Some("on disk"));
}
