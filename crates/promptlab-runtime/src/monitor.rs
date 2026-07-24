//! Runtime health monitoring.

use serde::{Deserialize, Serialize};

use crate::error::RuntimeResult;
use crate::supervisor::RuntimeSupervisor;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHealthReport {
    pub lifecycle_state: String,
    pub process_alive: bool,
    pub endpoint_reachable: bool,
    pub latency_ms: u64,
    pub memory_bytes: Option<u64>,
    pub gpu_memory_bytes: Option<u64>,
    pub model_loaded: bool,
    pub message: String,
}

pub struct RuntimeMonitor;

impl RuntimeMonitor {
    pub async fn check(
        supervisor: &mut RuntimeSupervisor,
        lifecycle_state: &str,
    ) -> RuntimeResult<RuntimeHealthReport> {
        let started = std::time::Instant::now();
        let healthy = supervisor.check_health().await.unwrap_or(false);
        let latency_ms = started.elapsed().as_millis() as u64;
        let model_loaded = supervisor.local_runtime().is_loaded();
        let runtime_alive = supervisor.is_process_alive_async().await;

        let message = if healthy && model_loaded {
            "runtime healthy — embedded libllama model loaded".into()
        } else if model_loaded {
            "model loaded but runtime health check failed".into()
        } else if runtime_alive {
            "embedded libllama initialized — no model loaded".into()
        } else {
            "runtime not initialized".into()
        };

        Ok(RuntimeHealthReport {
            lifecycle_state: lifecycle_state.to_string(),
            process_alive: runtime_alive,
            endpoint_reachable: healthy,
            latency_ms,
            memory_bytes: None,
            gpu_memory_bytes: None,
            model_loaded,
            message,
        })
    }
}
