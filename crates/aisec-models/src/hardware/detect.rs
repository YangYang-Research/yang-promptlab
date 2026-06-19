use std::process::Command;

use tracing::debug;

use crate::error::{ModelError, ModelResult};
use crate::types::{GpuBackend, GpuDevice, HardwareProfile};

/// Detect host CPU, memory, and GPU capabilities.
pub fn detect_hardware() -> ModelResult<HardwareProfile> {
    let os = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();
    let cpu_cores = detect_cpu_cores();
    let total_memory_bytes = detect_total_memory()?;
    let mut gpus = detect_gpus();

    if gpus.is_empty() {
        gpus.extend(fallback_gpu(&os, &arch));
    }

    debug!(
        os = %os,
        arch = %arch,
        cpu_cores,
        total_memory_bytes,
        gpu_count = gpus.len(),
        "hardware profile detected"
    );

    Ok(HardwareProfile {
        os,
        arch,
        cpu_cores,
        total_memory_bytes,
        gpus,
    })
}

fn detect_cpu_cores() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

fn detect_total_memory() -> ModelResult<u64> {
    #[cfg(target_os = "macos")]
    {
        use sysctl::{Ctl, CtlValue, Sysctl};
        let value = Ctl::new("hw.memsize")
            .map_err(|e| ModelError::Hardware(e.to_string()))?
            .value()
            .map_err(|e| ModelError::Hardware(e.to_string()))?;
        let mem = match value {
            CtlValue::U64(v) | CtlValue::Ulong(v) => v,
            CtlValue::S64(v) | CtlValue::Long(v) if v >= 0 => v as u64,
            other => {
                return Err(ModelError::Hardware(format!(
                    "unexpected hw.memsize sysctl value: {other:?}"
                )));
            }
        };
        return Ok(mem);
    }

    #[cfg(target_os = "linux")]
    {
        let content = std::fs::read_to_string("/proc/meminfo")
            .map_err(|e| ModelError::Hardware(e.to_string()))?;
        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                let kb: u64 = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                return Ok(kb * 1024);
            }
        }
    }

    Ok(8 * 1024 * 1024 * 1024)
}

fn detect_gpus() -> Vec<GpuDevice> {
    let mut devices = Vec::new();

    if let Some(nvidia) = detect_nvidia_smi() {
        devices.extend(nvidia);
    }

    #[cfg(target_os = "macos")]
    {
        devices.extend(detect_macos_gpus());
    }

    devices
}

fn detect_nvidia_smi() -> Option<Vec<GpuDevice>> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total,driver_version",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();
    for line in stdout.lines() {
        let parts: Vec<_> = line.split(',').map(|s| s.trim()).collect();
        if parts.is_empty() {
            continue;
        }
        let name = parts[0].to_string();
        let vram_mb: u64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        devices.push(GpuDevice {
            name,
            vendor: Some("NVIDIA".into()),
            vram_bytes: Some(vram_mb * 1024 * 1024),
            backend: GpuBackend::Cuda,
        });
    }

    if devices.is_empty() {
        None
    } else {
        Some(devices)
    }
}

#[cfg(target_os = "macos")]
fn detect_macos_gpus() -> Vec<GpuDevice> {
    let output = Command::new("system_profiler")
        .args(["SPDisplaysDataType", "-json"])
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                return parse_system_profiler_gpus(&json);
            }
        }
    }

    // Apple Silicon fallback
    if std::env::consts::ARCH == "aarch64" {
        return vec![GpuDevice {
            name: "Apple Silicon GPU".into(),
            vendor: Some("Apple".into()),
            vram_bytes: None,
            backend: GpuBackend::Metal,
        }];
    }

    vec![]
}

#[cfg(not(target_os = "macos"))]
fn detect_macos_gpus() -> Vec<GpuDevice> {
    vec![]
}

#[cfg(target_os = "macos")]
fn parse_system_profiler_gpus(json: &serde_json::Value) -> Vec<GpuDevice> {
    let mut devices = Vec::new();
    if let Some(arr) = json.get("SPDisplaysDataType").and_then(|v| v.as_array()) {
        for display in arr {
            if let Some(items) = display.get("_items").and_then(|v| v.as_array()) {
                for item in items {
                    let name = item
                        .get("_name")
                        .or_else(|| item.get("sppci_model"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("GPU")
                        .to_string();
                    devices.push(GpuDevice {
                        name,
                        vendor: Some("Apple".into()),
                        vram_bytes: None,
                        backend: GpuBackend::Metal,
                    });
                }
            }
        }
    }
    devices
}

fn fallback_gpu(os: &str, arch: &str) -> Vec<GpuDevice> {
    if os == "macos" && arch == "aarch64" {
        vec![GpuDevice {
            name: "Apple Silicon (Metal)".into(),
            vendor: Some("Apple".into()),
            vram_bytes: None,
            backend: GpuBackend::Metal,
        }]
    } else {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_returns_profile() {
        let profile = detect_hardware().unwrap();
        assert!(profile.cpu_cores >= 1);
        assert!(profile.total_memory_bytes > 0);
        assert!(!profile.os.is_empty());
    }
}
