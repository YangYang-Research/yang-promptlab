use aisec_storage::{
    CreateProject, Database, ProjectRepository,
};

#[tokio::test]
async fn storage_persists_to_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    let db_path = temp.path().join("aisec.db");
    let url = format!("sqlite://{}", db_path.display());

    let db = Database::connect(&url).await.expect("connect");
    let project = db
        .repositories()
        .projects()
        .create(CreateProject {
            name: "File-backed".into(),
            description: None,
        })
        .await
        .expect("create");

    drop(db);

    let db = Database::connect(&url).await.expect("reconnect");
    let loaded = db
        .repositories()
        .projects()
        .get(&project.id)
        .await
        .expect("get");

    assert_eq!(loaded.name, "File-backed");
}
