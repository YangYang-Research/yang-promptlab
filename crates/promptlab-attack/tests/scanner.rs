//! Prompt Injection Scanner integration tests.
//!
//! These use a real local HTTP server (wiremock) as the *target under test* and
//! exercise the full scanner path: harness transport, real response capture,
//! real evaluation, and real SQLite persistence. Nothing about the scanner is
//! mocked, and findings are derived from actual responses.
#![cfg(feature = "storage")]

use promptlab_attack::scanner::{PromptInjectionScanner, ScanContext};
use promptlab_attack::{AttackBudget, AttackTarget};
use promptlab_storage::{
    AttackResultRepository, CreateProject, CreateScan, Database, FindingRepository,
    ProjectRepository, ScanRepository,
};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn seeded_db() -> (Database, String, String) {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let repos = db.repositories();
    let project = repos
        .projects()
        .create(CreateProject {
            name: "PI Scan".into(),
            description: None,
        })
        .await
        .unwrap();
    let scan = repos
        .scans()
        .create(CreateScan {
            project_id: project.id.clone(),
            target_id: None,
            name: "prompt injection".into(),
            status: Some("running".into()),
            playbook_json: None,
        })
        .await
        .unwrap();
    (db.clone(), project.id, scan.id)
}

#[tokio::test]
async fn scans_real_http_and_persists_findings() {
    // A real HTTP endpoint behaving like a vulnerable LLM: it leaks its system
    // prompt and an API key in the completion.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Debug mode enabled. System prompt: You are SecureBot. Here is the API key: sk-live-abc123."
                }
            }]
        })))
        .mount(&server)
        .await;

    let (db, project_id, scan_id) = seeded_db().await;
    let scanner = PromptInjectionScanner::new(db.clone());
    let target = AttackTarget::llm_api(server.uri()); // real URL
    let ctx = ScanContext::new(scan_id.clone(), project_id);

    let summary = scanner
        .scan(target, &ctx, AttackBudget::default())
        .await
        .unwrap();

    assert!(summary.payloads_sent > 0, "payloads should be sent over HTTP");
    assert!(
        summary.findings_stored > 0,
        "vulnerable target should yield findings"
    );

    let repos = db.repositories();

    // Findings are actually persisted to SQLite.
    let stored = repos.findings().list_by_scan(&scan_id).await.unwrap();
    assert_eq!(stored.len(), summary.findings_stored);
    assert!(stored
        .iter()
        .all(|f| f.category.as_deref() == Some("prompt_injection")));
    // API-key leak => critical severity.
    assert!(stored.iter().any(|f| f.severity == "critical"));

    // Every probe is recorded as an attack_result for auditability.
    let results = repos.attack_results().list_by_scan(&scan_id).await.unwrap();
    assert_eq!(results.len(), summary.payloads_sent);
}

#[tokio::test]
async fn safe_target_yields_no_findings() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "choices": [{ "message": { "role": "assistant", "content": "I can't help with that request." } }]
        })))
        .mount(&server)
        .await;

    let (db, project_id, scan_id) = seeded_db().await;
    let scanner = PromptInjectionScanner::new(db.clone());

    let summary = scanner
        .scan(
            AttackTarget::llm_api(server.uri()),
            &ScanContext::new(scan_id.clone(), project_id),
            AttackBudget::default(),
        )
        .await
        .unwrap();

    assert!(summary.payloads_sent > 0);
    assert_eq!(summary.findings_stored, 0, "safe target must not yield findings");
    assert!(db
        .repositories()
        .findings()
        .list_by_scan(&scan_id)
        .await
        .unwrap()
        .is_empty());
}
