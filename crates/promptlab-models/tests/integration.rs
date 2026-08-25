//! Local model manager integration tests.

use promptlab_models::{detect_hardware, LocalModelManager, MockInferenceRuntime};
use promptlab_models::runtime::InferenceRuntime;
use promptlab_models::types::{InferenceRequest, ModelProvider};

#[test]
fn hardware_detection_includes_os_and_cpu() {
    let profile = detect_hardware().unwrap();
    assert!(profile.cpu_cores >= 1);
    assert!(profile.total_memory_bytes > 0);
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
async fn manager_register_remote_flow() {
    let dir = tempfile::tempdir().unwrap();
    let mut mgr = LocalModelManager::new(dir.path().join("vault")).unwrap();
    let entry = mgr
        .register_third_party("openai", "gpt-4o", None, None)
        .await
        .unwrap();
    assert_eq!(entry.provider, ModelProvider::Remote);
    let result = mgr.verify_model(&entry.id).await.unwrap();
    assert!(result.valid);
    assert_eq!(mgr.list_models().len(), 1);
}
