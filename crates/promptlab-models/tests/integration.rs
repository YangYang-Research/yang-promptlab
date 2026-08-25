//! Local model manager integration tests.

use promptlab_models::{
    detect_hardware, huggingface_url, DownloadManager, DownloadStatus, LocalModelManager,
    MockInferenceRuntime, VerificationEngine,
};
use promptlab_models::runtime::InferenceRuntime;
use promptlab_models::types::InferenceRequest;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn download_and_verify_gguf() {
    let server = MockServer::start().await;
    let body = b"GGUF-stub-model-bytes-for-test";

    Mock::given(method("GET"))
        .and(path("/resolve/main/tiny.gguf"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("tiny.gguf");

    let progress = DownloadManager::with_defaults()
        .download(
            &format!("{}/resolve/main/tiny.gguf", server.uri()),
            &dest,
        )
        .await
        .unwrap();
    assert_eq!(progress.status, DownloadStatus::Completed);

    let (hash, _) = VerificationEngine::hash_file(&dest).await.unwrap();
    let verified = VerificationEngine::verify_file(&dest, Some(&hash))
        .await
        .unwrap();
    assert!(verified.valid);
}

#[test]
fn hardware_detection_includes_os_and_cpu() {
    let profile = detect_hardware().unwrap();
    assert!(profile.cpu_cores >= 1);
    assert!(profile.total_memory_bytes > 0);
}

#[test]
fn huggingface_url_format() {
    let url = huggingface_url("org/model", "file.gguf", None);
    assert!(url.contains("resolve/main"));
}

#[tokio::test]
async fn mock_inference_runtime() {
    let mut runtime = MockInferenceRuntime::new("generated");
    runtime
        .load_model(std::path::Path::new("/tmp/fake.gguf"))
        .await
        .unwrap();
    let resp = runtime
        .complete(InferenceRequest {
            system: None,
            prompt: "test".into(),
            max_tokens: 64,
            temperature: 0.0,
        })
        .await
        .unwrap();
    assert!(resp.text.contains("generated"));
}

#[tokio::test]
async fn catalog_install_rejected_without_builtin_registry() {
    let dir = tempfile::tempdir().unwrap();
    let mut mgr = LocalModelManager::new(dir.path().join("vault")).unwrap();
    let err = mgr
        .install_catalog("any-id", None)
        .await
        .expect_err("builtin catalog removed");
    assert!(err.to_string().contains("removed"));
}

#[tokio::test]
async fn manager_import_verify_flow() {
    let dir = tempfile::tempdir().unwrap();
    let model = dir.path().join("local.gguf");
    tokio::fs::write(&model, b"local-gguf").await.unwrap();

    let mut mgr = LocalModelManager::new(dir.path().join("vault")).unwrap();
    let entry = mgr.import_local("local", &model).await.unwrap();
    let result = mgr.verify_model(&entry.id).await.unwrap();
    assert!(result.valid);
    assert!(mgr.list_models()[0].verified);
}
