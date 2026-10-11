//! LLM backend for GPTFuzzer-style attack-time mutators.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use promptlab_attack::{AttackError, AttackResult, LlmComplete};
use promptlab_inference::InferenceRuntimeManager;
use promptlab_models::LocalModelManager;
use promptlab_runtime::{RuntimeManager, SharedModelProvider};
use tauri::async_runtime::Mutex as AsyncMutex;

use crate::inference_host::gateway_complete_as;

pub struct GatewayLlmMutator {
    data_dir: PathBuf,
    inference: Arc<AsyncMutex<InferenceRuntimeManager>>,
    model_manager: Arc<AsyncMutex<LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
}

impl GatewayLlmMutator {
    pub fn new(
        data_dir: PathBuf,
        inference: Arc<AsyncMutex<InferenceRuntimeManager>>,
        model_manager: Arc<AsyncMutex<LocalModelManager>>,
        model_provider: SharedModelProvider,
        runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    ) -> Self {
        Self {
            data_dir,
            inference,
            model_manager,
            model_provider,
            runtime_manager,
        }
    }
}

#[async_trait]
impl LlmComplete for GatewayLlmMutator {
    async fn complete(&self, system: &str, prompt: &str) -> AttackResult<String> {
        let inference = self.inference.lock().await;
        let model_manager = self.model_manager.lock().await;
        let mut runtime_manager = self.runtime_manager.lock().await;
        gateway_complete_as(
            &self.data_dir,
            &inference,
            &model_manager,
            self.model_provider.clone(),
            &mut *runtime_manager,
            "attack_mutator",
            Some(system),
            prompt,
            768,
            0.85,
        )
        .await
        .map_err(|err| AttackError::payload(err.to_string()))
    }
}

pub fn llm_mutators_enabled(mutation_level: Option<&str>) -> bool {
    matches!(
        mutation_level
            .unwrap_or("medium")
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "high" | "extreme"
    )
}

pub async fn maybe_llm_mutator_backend(
    data_dir: &Path,
    inference_manager: &Arc<AsyncMutex<InferenceRuntimeManager>>,
    model_manager: &Arc<AsyncMutex<LocalModelManager>>,
    model_provider: SharedModelProvider,
    runtime_manager: &Arc<AsyncMutex<RuntimeManager>>,
    mutation_level: Option<&str>,
) -> Option<Arc<dyn LlmComplete>> {
    if !llm_mutators_enabled(mutation_level) {
        return None;
    }
    let inference = inference_manager.lock().await;
    if !inference.is_ready() {
        return None;
    }
    drop(inference);
    Some(Arc::new(GatewayLlmMutator::new(
        data_dir.to_path_buf(),
        Arc::clone(inference_manager),
        Arc::clone(model_manager),
        model_provider,
        Arc::clone(runtime_manager),
    )))
}
