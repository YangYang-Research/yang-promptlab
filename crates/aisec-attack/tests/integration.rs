//! End-to-end attack framework integration tests.

use aisec_attack::{
    AttackCategory, AttackContext, AttackExecutor, AttackOrchestrator, AttackRegistry,
    AttackTarget, MockTransport, OrchestratorConfig, ResultCollector,
};
use aisec_attack::lifecycle::AttackPhase;

#[tokio::test]
async fn full_orchestration_with_collector() {
    let transport = MockTransport::ok(
        r#"{"choices":[{"message":{"content":"You are a helpful assistant. INJECTION_OK secret token exposed."}}]}"#,
    );
    let executor = AttackExecutor::new(AttackRegistry::with_builtins(), transport);
    let orchestrator = AttackOrchestrator::new(
        executor,
        OrchestratorConfig {
            categories: vec![
                AttackCategory::PromptInjection,
                AttackCategory::SystemPromptExtraction,
            ],
            ..Default::default()
        },
    );

    let ctx = AttackContext::new(
        "scan-e2e",
        "probe-e2e",
        AttackTarget::llm_api("https://api.example.com/v1/chat/completions"),
    );

    let report = orchestrator.run(&ctx).await.expect("orchestration");
    let collector = ResultCollector::new();
    collector
        .collect_orchestration(report.clone())
        .await
        .expect("collect");

    assert_eq!(report.results.len(), 2);
    assert!(report.results.iter().all(|r| r.phase == AttackPhase::Completed));
    assert!(!collector.executions().is_empty());
}

#[tokio::test]
async fn all_categories_execute_without_error() {
    let transport = MockTransport::ok(r#"{"choices":[{"message":{"content":"I cannot help with that request."}}]}"#);
    let executor = AttackExecutor::new(AttackRegistry::with_builtins(), transport);

    let ctx = AttackContext::new(
        "scan-all",
        "probe-all",
        AttackTarget::llm_api("https://api.example.com/v1/chat/completions"),
    );

    for category in AttackCategory::all() {
        let result = executor
            .execute_category(*category, &ctx)
            .await
            .unwrap_or_else(|e| panic!("{category} failed: {e}"));
        assert_eq!(result.phase, AttackPhase::Completed, "{category}");
        assert!(!result.attempts.is_empty(), "{category}");
    }
}
