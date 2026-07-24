use async_trait::async_trait;
use promptlab_runtime::RuntimeSupervisor;

use super::RuntimeAdapter;
use crate::error::{InferenceError, InferenceResult};

/// Runtime adapter for embedded libllama — hides supervisor details from RuntimeManager.
pub struct LocalRuntimeAdapterBridge<'a> {
    supervisor: &'a mut RuntimeSupervisor,
}

impl<'a> LocalRuntimeAdapterBridge<'a> {
    pub fn new(supervisor: &'a mut RuntimeSupervisor) -> Self {
        Self { supervisor }
    }
}

#[async_trait]
impl RuntimeAdapter for LocalRuntimeAdapterBridge<'_> {
    fn runtime_name(&self) -> &str {
        "embedded-libllama"
    }

    async fn ensure_running(&mut self) -> InferenceResult<()> {
        self.supervisor
            .ensure_running()
            .await
            .map_err(|e| InferenceError::Internal(e.to_string()))
    }

    async fn ensure_model_loaded(&mut self, model_path: &std::path::Path) -> InferenceResult<()> {
        self.supervisor
            .ensure_model_loaded(model_path)
            .await
            .map_err(|e| InferenceError::Internal(e.to_string()))
    }

    async fn health(&mut self) -> InferenceResult<bool> {
        self.supervisor
            .check_health()
            .await
            .map_err(|e| InferenceError::Internal(e.to_string()))
    }
}
