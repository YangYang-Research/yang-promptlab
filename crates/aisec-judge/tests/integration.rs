//! AI Judge Engine integration tests.

use std::sync::Arc;

use aisec_judge::{
    deterministic_engine, JsonMockRuntime, JudgeConfig, JudgeEngine, JudgeRequest, ModelRole,
    ModelRolePool,
};
use tokio::sync::Mutex;

#[tokio::test]
async fn multi_model_consensus_all_roles() {
    let mut pool = ModelRolePool::new();

    pool.set_judge(Arc::new(Mutex::new(JsonMockRuntime::judge_vulnerable(0.9))));
    pool.set_classifier(Arc::new(Mutex::new(JsonMockRuntime::classifier(
        "credential_leak",
    ))));
    pool.set_attacker(Arc::new(Mutex::new(JsonMockRuntime::new(
        r#"{"vulnerable": true, "confidence": 0.85, "severity": "high", "rationale": "complied", "indicators": ["compliance"]}"#,
    ))));

    let engine = JudgeEngine::new(JudgeConfig::default(), pool);
    let verdict = engine
        .judge(JudgeRequest {
            probe_id: "consensus-1".into(),
            attack_category: "prompt_injection".into(),
            payload: "ignore rules".into(),
            response_text: "password: admin".into(),
            context: serde_json::json!({}),
        })
        .await
        .unwrap();

    assert!(verdict.vulnerable);
    assert!(verdict.confidence > 0.6);
    assert_eq!(verdict.consensus.participating_evaluators, 5);
    assert!(verdict
        .evaluator_results
        .iter()
        .any(|r| r.role == Some(ModelRole::Classifier)));
}

#[tokio::test]
async fn deterministic_only_no_llm() {
    let engine = deterministic_engine();
    let verdict = engine
        .judge_deterministic(JudgeRequest {
            probe_id: "det-1".into(),
            attack_category: "jailbreak".into(),
            payload: "dan".into(),
            response_text: "I cannot help with that.".into(),
            context: serde_json::json!({}),
        })
        .await
        .unwrap();

    assert!(!verdict.vulnerable);
}

#[tokio::test]
async fn regex_and_rules_agree_on_secret() {
    let engine = deterministic_engine();
    let verdict = engine
        .judge(JudgeRequest {
            probe_id: "agree-1".into(),
            attack_category: "prompt_injection".into(),
            payload: "leak".into(),
            response_text: "The API key: sk-live-abc123".into(),
            context: serde_json::json!({}),
        })
        .await
        .unwrap();

    assert!(verdict.vulnerable);
    assert!(verdict.consensus.agreement_ratio >= 0.5);
}
