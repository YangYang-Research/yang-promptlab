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
    pub async fn check(supervisor: &mut RuntimeSupervisor, lifecycle_state: &str) -> RuntimeResult<RuntimeHealthReport> {
        let started = std::time::Instant::now();
        let endpoint_reachable = supervisor.check_health().await.unwrap_or(false);
        let latency_ms = started.elapsed().as_millis() as u64;
        let model_loaded = supervisor.llama_runtime().is_loaded();
        let process_alive = model_loaded || supervisor.is_process_alive();

        let memory_bytes = if let Some(pid) = supervisor.pid().await {
            process_memory_bytes(Some(pid))
        } else {
            None
        };
        let gpu_memory_bytes = None;

        let message = if endpoint_reachable && model_loaded {
            "runtime healthy — inference endpoint reachable with model loaded".into()
        } else if endpoint_reachable {
            "runtime idle — endpoint reachable, awaiting model load from Models module".into()
        } else if supervisor.binary_available() {
            "runtime installed — llama-server process not running".into()
        } else {
            "runtime not installed".into()
        };

        Ok(RuntimeHealthReport {
            lifecycle_state: lifecycle_state.to_string(),
            process_alive,
            endpoint_reachable,
            latency_ms,
            memory_bytes,
            gpu_memory_bytes,
            model_loaded,
            message,
        })
    }
}

fn process_memory_bytes(pid: Option<u32>) -> Option<u64> {
    let pid = pid?;
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let kb: u64 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .ok()?;
        return Some(kb * 1024);
    }
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                let kb: u64 = rest.trim().trim_end_matches(" kB").parse().ok()?;
                return Some(kb * 1024);
            }
        }
    }
    let _ = pid;
    None
}
