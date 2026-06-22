//! Hardware detection with persistence (first launch + manual refresh only).

use std::path::{Path, PathBuf};

use aisec_models::detect_hardware;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::error::{RuntimeError, RuntimeResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHardwareProfile {
    pub os: String,
    pub arch: String,
    pub cpu: String,
    pub cpu_cores: usize,
    pub ram_bytes: u64,
    pub gpu_vendor: Option<String>,
    pub gpu_name: Option<String>,
    pub vram_bytes: Option<u64>,
    pub cuda: bool,
    pub metal: bool,
    pub vulkan: bool,
    pub avx2: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub detected_at: OffsetDateTime,
}

pub struct HardwareDetector {
    data_dir: PathBuf,
}

impl HardwareDetector {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    fn profile_path(&self) -> PathBuf {
        self.data_dir.join("runtime").join("hardware.json")
    }

    pub async fn load(&self) -> RuntimeResult<Option<RuntimeHardwareProfile>> {
        let path = self.profile_path();
        if !path.is_file() {
            return Ok(None);
        }
        let raw = tokio::fs::read_to_string(&path)
            .await
            .map_err(|err| RuntimeError::Config(err.to_string()))?;
        serde_json::from_str(&raw).map_err(|err| RuntimeError::Config(err.to_string()))
    }

    pub async fn detect_and_persist(&self) -> RuntimeResult<RuntimeHardwareProfile> {
        let base = detect_hardware().map_err(|err| RuntimeError::Config(err.to_string()))?;
        let primary = base.primary_gpu();
        let cuda = base
            .gpus
            .iter()
            .any(|gpu| matches!(gpu.backend, aisec_models::GpuBackend::Cuda));
        let metal = base
            .gpus
            .iter()
            .any(|gpu| matches!(gpu.backend, aisec_models::GpuBackend::Metal))
            || (base.os == "macos" && base.arch == "aarch64");
        let vulkan = base
            .gpus
            .iter()
            .any(|gpu| matches!(gpu.backend, aisec_models::GpuBackend::Vulkan));

        let profile = RuntimeHardwareProfile {
            os: base.os.clone(),
            arch: base.arch.clone(),
            cpu: format!("{}-core host", base.cpu_cores),
            cpu_cores: base.cpu_cores,
            ram_bytes: base.total_memory_bytes,
            gpu_vendor: primary.and_then(|gpu| gpu.vendor.clone()),
            gpu_name: primary.map(|gpu| gpu.name.clone()),
            vram_bytes: primary.and_then(|gpu| gpu.vram_bytes),
            cuda,
            metal,
            vulkan,
            avx2: detect_avx2(),
            detected_at: OffsetDateTime::now_utc(),
        };

        if let Some(parent) = self.profile_path().parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|err| RuntimeError::Config(err.to_string()))?;
        }
        let raw = serde_json::to_string_pretty(&profile)
            .map_err(|err| RuntimeError::Config(err.to_string()))?;
        tokio::fs::write(self.profile_path(), raw)
            .await
            .map_err(|err| RuntimeError::Config(err.to_string()))?;

        Ok(profile)
    }

    pub async fn ensure_profile(&self) -> RuntimeResult<RuntimeHardwareProfile> {
        if let Some(profile) = self.load().await? {
            return Ok(profile);
        }
        self.detect_and_persist().await
    }
}

fn detect_avx2() -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return std::arch::is_x86_feature_detected!("avx2");
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        false
    }
}
