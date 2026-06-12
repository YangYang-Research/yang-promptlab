//! Prompt Injection Scanner demo against a live HTTP target (no mocks).
//!
//! Run:
//!   python3 scripts/vuln-chatbot-target.py
//!   cargo run -p aisec-attack --features storage --example scan_prompt_injection

use aisec_attack::scanner::{PromptInjectionScanner, ScanContext};
use aisec_attack::{AttackBudget, AttackTarget};
use aisec_storage::{CreateProject, CreateScan, Database, FindingRepository, ProjectRepository, ScanRepository};

#[tokio::main]
async fn main() {
    let url =
        std::env::var("PI_TARGET").unwrap_or_else(|_| "http://localhost:3300/v1/chat/completions".into());

    // Persistent storage (migrations run automatically).
    let db = Database::connect("sqlite::memory:").await.expect("db");
    let repos = db.repositories();
    let project = repos
        .projects()
        .create(CreateProject {
            name: "PI Demo".into(),
            description: Some("prompt injection scan".into()),
        })
        .await
        .expect("project");
    let scan = repos
        .scans()
        .create(CreateScan {
            project_id: project.id.clone(),
            target_id: None,
            name: "prompt injection scan".into(),
            status: Some("running".into()),
            playbook_json: None,
        })
        .await
        .expect("scan");

    println!("=== AISec Prompt Injection Scanner ===");
    println!("Target: {url}\n");

    let scanner = PromptInjectionScanner::new(db.clone());
    let target = AttackTarget::llm_api(&url);
    let ctx = ScanContext::new(scan.id.clone(), project.id.clone());

    let summary = scanner
        .scan(target, &ctx, AttackBudget::default())
        .await
        .expect("scan failed (is the target server running?)");

    println!("Payloads sent (real HTTP): {}", summary.payloads_sent);
    println!("Responses captured:        {}", summary.responses_captured);
    println!("Findings stored:           {}", summary.findings_stored);
    println!("Highest severity:          {:?}\n", summary.highest_severity);

    // Read the findings back out of SQLite to prove they were persisted.
    let findings = repos.findings().list_by_scan(&scan.id).await.expect("findings");
    println!("--- Findings persisted in SQLite ({}) ---", findings.len());
    for f in &findings {
        println!("  [{}] {}  ({})", f.severity, f.title, f.category.as_deref().unwrap_or("-"));
    }
}
