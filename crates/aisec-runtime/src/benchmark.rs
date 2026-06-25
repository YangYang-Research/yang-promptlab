//! Runtime inference benchmark (requires model loaded via Models module).

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::error::RuntimeResult;
use crate::local_runtime_adapter::InferRequest;
use crate::supervisor::RuntimeSupervisor;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeBenchmarkResult {
    pub ok: bool,
    pub latency_ms: u64,
    pub tokens_per_sec: f64,
    pub tokens_predicted: u32,
    pub memory_bytes: Option<u64>,
    pub gpu_memory_bytes: Option<u64>,
    pub message: String,
    #[serde(with = "time::serde::rfc3339")]
    pub measured_at: OffsetDateTime,
}

pub struct RuntimeBenchmark;

impl RuntimeBenchmark {
    pub async fn run(supervisor: &RuntimeSupervisor) -> RuntimeResult<RuntimeBenchmarkResult> {
        let runtime = supervisor.local_runtime();
        if !runtime.is_loaded() {
            return Ok(RuntimeBenchmarkResult {
                ok: false,
                latency_ms: 0,
                tokens_per_sec: 0.0,
                tokens_predicted: 0,
                memory_bytes: None,
                gpu_memory_bytes: None,
                message: "No model loaded — activate a model from the Models module first".into(),
                measured_at: OffsetDateTime::now_utc(),
            });
        }

        let response = supervisor
            .infer(InferRequest {
                prompt: "Benchmark prompt. Reply with one word: OK".into(),
                max_tokens: 8,
                temperature: 0.0,
            })
            .await?;

        let duration_ms = response.duration_ms.max(1);
        let tokens_per_sec = response.tokens_predicted as f64 / (duration_ms as f64 / 1000.0);

        Ok(RuntimeBenchmarkResult {
            ok: !response.text.is_empty(),
            latency_ms: duration_ms,
            tokens_per_sec,
            tokens_predicted: response.tokens_predicted,
            memory_bytes: None,
            gpu_memory_bytes: None,
            message: "Benchmark completed".into(),
            measured_at: OffsetDateTime::now_utc(),
        })
    }
}
