//! Hardware detection with SQLite persistence.

use std::path::{Path, PathBuf};

use promptlab_models::detect_hardware;
use promptlab_storage::{Database, HardwareProfileRepository};
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
    /// Free space on the volume that holds `data_dir` (models / runtime cache).
    #[serde(default)]
    pub disk_free_bytes: Option<u64>,
    #[serde(with = "time::serde::rfc3339")]
    pub detected_at: OffsetDateTime,
}

pub struct HardwareDetector {
    data_dir: PathBuf,
    db: Option<Database>,
}

impl HardwareDetector {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            db: None,
        }
    }

    pub fn with_db(data_dir: impl Into<PathBuf>, db: Database) -> Self {
        Self {
            data_dir: data_dir.into(),
            db: Some(db),
        }
    }

    async fn load_from_db(&self, db: &Database) -> RuntimeResult<Option<RuntimeHardwareProfile>> {
        let record = db
            .repositories()
            .hardware_profile()
            .get()
            .await
            .map_err(|err| RuntimeError::Config(err.to_string()))?;
        match record {
            Some(row) => {
                let profile = serde_json::from_str(&row.profile_json)
                    .map_err(|err| RuntimeError::Config(err.to_string()))?;
                Ok(Some(profile))
            }
            None => Ok(None),
        }
    }

    async fn save_to_db(&self, db: &Database, profile: &RuntimeHardwareProfile) -> RuntimeResult<()> {
        let raw = serde_json::to_string(profile)
            .map_err(|err| RuntimeError::Config(err.to_string()))?;
        db.repositories()
            .hardware_profile()
            .upsert(&raw)
            .await
            .map_err(|err| RuntimeError::Config(err.to_string()))?;
        Ok(())
    }

    pub async fn load(&self) -> RuntimeResult<Option<RuntimeHardwareProfile>> {
        let Some(db) = &self.db else {
            return Ok(None);
        };
        self.load_from_db(db).await
    }

    pub async fn detect_and_persist(&self) -> RuntimeResult<RuntimeHardwareProfile> {
        let base = detect_hardware().map_err(|err| RuntimeError::Config(err.to_string()))?;
        let primary = base.primary_gpu();
        let cuda = base
            .gpus
            .iter()
            .any(|gpu| matches!(gpu.backend, promptlab_models::GpuBackend::Cuda));
        let metal = base
            .gpus
            .iter()
            .any(|gpu| matches!(gpu.backend, promptlab_models::GpuBackend::Metal))
            || (base.os == "macos" && base.arch == "aarch64");
        let vulkan = base
            .gpus
            .iter()
            .any(|gpu| matches!(gpu.backend, promptlab_models::GpuBackend::Vulkan));

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
            disk_free_bytes: detect_free_disk_bytes(&self.data_dir),
            detected_at: OffsetDateTime::now_utc(),
        };

        if let Some(db) = &self.db {
            self.save_to_db(db, &profile).await?;
        }

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

fn detect_free_disk_bytes(path: &Path) -> Option<u64> {
    let probe = if path.exists() {
        path.to_path_buf()
    } else {
        path.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    };

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;

        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetDiskFreeSpaceExW(
                directory: *const u16,
                free_bytes_available: *mut u64,
                total_bytes: *mut u64,
                total_free_bytes: *mut u64,
            ) -> i32;
        }

        let wide: Vec<u16> = probe
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut free_available = 0u64;
        let mut total = 0u64;
        let mut total_free = 0u64;
        let ok = unsafe {
            GetDiskFreeSpaceExW(
                wide.as_ptr(),
                &mut free_available,
                &mut total,
                &mut total_free,
            )
        };
        if ok != 0 && free_available > 0 {
            return Some(free_available);
        }
        return None;
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use std::ffi::CString;
        use std::mem::MaybeUninit;

        let c_path = CString::new(probe.to_str()?).ok()?;
        let mut stat = MaybeUninit::<libc::statvfs>::uninit();
        let rc = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
        if rc != 0 {
            return None;
        }
        let stat = unsafe { stat.assume_init() };
        let free = stat.f_bavail as u64 * stat.f_frsize as u64;
        if free > 0 {
            Some(free)
        } else {
            None
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        let _ = probe;
        None
    }
}
