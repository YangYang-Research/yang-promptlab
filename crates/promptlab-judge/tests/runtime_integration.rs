//! Judge runtime bridge integration tests.

use std::sync::Arc;

use promptlab_judge::{
    build_judge_engine, JudgeMode, JudgeProviderConfig, JudgeRequest, JudgeRuntimeContext,
    LocalProviderSettings,
};
use promptlab_models::types::{InferenceRequest, InferenceResponse};
use promptlab_runtime::provider::{ModelProvider, ModelProviderHealth};
use async_trait::async_trait;

struct JsonJudgeProvider {
    json: String,
}

#[async_trait]
impl ModelProvider for JsonJudgeProvider {
    async fn list_models(&self) -> promptlab_runtime::RuntimeResult<Vec<String>> {
        Ok(vec!["vault-judge".into()])
    }

    async fn install_model(&self, _model_id: &str) -> promptlab_runtime::RuntimeResult<()> {
        Ok(())
    }

    async fn remove_model(&self, _model_id: &str) -> promptlab_runtime::RuntimeResult<()> {
        Ok(())
    }

    async fn run_inference(&self, _model_id: &str, _prompt: &str) -> promptlab_runtime::RuntimeResult<String> {
        Ok(self.json.clone())
    }

    async fn complete_for_model(
        &self,
        _model_id: &str,
        _request: &InferenceRequest,
    ) -> promptlab_runtime::RuntimeResult<InferenceResponse> {
        Ok(InferenceResponse {
            text: self.json.clone(),
            tokens_predicted: 32,
            duration_ms: 5,
        })
    }

    async fn health(&self) -> promptlab_runtime::RuntimeResult<ModelProviderHealth> {
        Ok(ModelProviderHealth {
            healthy: true,
            message: "mock runtime provider".into(),
        })
    }
}

#[tokio::test]
async fn local_judge_routes_through_model_provider_runtime() {
    let json = r#"{"vulnerable": true, "confidence": 0.93, "severity": "high", "category": "prompt_injection", "rationale": "credential leak", "indicators": ["api_key"]}"#;
    let provider: Arc<dyn ModelProvider> = Arc::new(JsonJudgeProvider {
        json: json.into(),
    });
    let runtime = JudgeRuntimeContext::new(provider, "vault-judge");

    let mut config = JudgeProviderConfig::default();
    config.mode = JudgeMode::LocalLlm;
    config.local = LocalProviderSettings {
        vault_model_id: Some("vault-judge".into()),
        model: "vault-judge".into(),
        ..LocalProviderSettings::default()
    };

    let engine = build_judge_engine(&config, Some(runtime))
        .await
        .expect("engine");
    let verdict = engine
        .judge(JudgeRequest {
            probe_id: "runtime-bridge".into(),
            attack_category: "prompt_injection".into(),
            payload: "ignore rules".into(),
            response_text: "sk-live-123".into(),
            context: serde_json::json!({}),
        })
        .await
        .expect("verdict");

    assert!(verdict.vulnerable);
    let structured = verdict.to_structured_output();
    assert_eq!(structured.mode, "local_llm");
    assert!(structured
        .evaluator_results
        .iter()
        .any(|result| result.structured.is_some()));
    assert!(verdict.to_json_string().unwrap().contains("\"vulnerable\""));
}
