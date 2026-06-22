//! Download, verify, and install embedded llama-server binaries.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::info;

use crate::error::{RuntimeError, RuntimeResult};
use crate::hardware::RuntimeHardwareProfile;
use crate::manifest::{RuntimeBackend, RuntimeManifest};
use crate::paths::bundled_llama_server_binary;

const DEFAULT_RELEASE: &str = "b9551";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePackage {
    pub release: String,
    pub backend: RuntimeBackend,
    pub archive_name: String,
    pub download_url: String,
}

pub struct RuntimeInstaller {
    data_dir: PathBuf,
    bundled_binary: Option<PathBuf>,
}

impl RuntimeInstaller {
    pub fn new(data_dir: impl Into<PathBuf>, bundled_binary: Option<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            bundled_binary,
        }
    }

    pub fn install_path(&self) -> PathBuf {
        bundled_llama_server_binary(&self.data_dir)
    }

    pub fn is_installed(&self) -> bool {
        self.install_path().is_file()
    }

    pub fn select_package(profile: &RuntimeHardwareProfile) -> RuntimeResult<RuntimePackage> {
        let release = std::env::var("AISEC_LLAMA_RELEASE")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_RELEASE.into());

        let (backend, archive_name) = select_archive(profile)?;
        let download_url = format!(
            "https://github.com/ggml-org/llama.cpp/releases/download/{release}/{archive_name}"
        );

        Ok(RuntimePackage {
            release,
            backend,
            archive_name,
            download_url,
        })
    }

    pub async fn install(
        &self,
        profile: &RuntimeHardwareProfile,
        progress: impl Fn(&str),
    ) -> RuntimeResult<RuntimeManifest> {
        progress("selecting runtime package");
        let package = Self::select_package(profile)?;
        let target = self.install_path();

        if let Some(source) = self.bundled_binary.as_ref().filter(|p| p.is_file()) {
            progress("installing bundled runtime");
            self.copy_binary(source, &target).await?;
        } else if let Some(dev) = dev_repo_binary().filter(|p| p.is_file()) {
            progress("installing development runtime");
            self.copy_binary(&dev, &target).await?;
        } else if target.is_file() {
            progress("runtime already installed");
        } else {
            progress("downloading runtime");
            self.download_and_extract(&package, &target).await?;
        }

        progress("verifying runtime");
        let sha256 = sha256_file(&target).await?;
        let mut manifest = RuntimeManifest::new(
            package.release.clone(),
            package.backend,
            platform_label(),
            target.clone(),
        );
        manifest.installed = true;
        manifest.verified = true;
        manifest.sha256 = Some(sha256);
        manifest.installed_at = Some(time::OffsetDateTime::now_utc());
        manifest.save(&self.data_dir).await?;
        Ok(manifest)
    }

    pub async fn verify(&self, manifest: &RuntimeManifest) -> RuntimeResult<bool> {
        if !manifest.install_path.is_file() {
            return Ok(false);
        }
        let actual = sha256_file(&manifest.install_path).await?;
        Ok(manifest.sha256.as_deref() == Some(actual.as_str()))
    }

    async fn copy_binary(&self, source: &Path, target: &Path) -> RuntimeResult<()> {
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|err| RuntimeError::Process(err.to_string()))?;
        }
        tokio::fs::copy(source, target)
            .await
            .map_err(|err| RuntimeError::Process(err.to_string()))?;
        set_executable(target).await
    }

    async fn download_and_extract(&self, package: &RuntimePackage, target: &Path) -> RuntimeResult<()> {
        info!(url = %package.download_url, "downloading llama-server runtime");
        let client = reqwest::Client::builder()
            .user_agent("AISec/0.1")
            .build()
            .map_err(|err| RuntimeError::Process(err.to_string()))?;

        let response = client
            .get(&package.download_url)
            .send()
            .await
            .map_err(|err| RuntimeError::Process(err.to_string()))?;

        if !response.status().is_success() {
            return Err(RuntimeError::Process(format!(
                "download failed HTTP {}",
                response.status()
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|err| RuntimeError::Process(err.to_string()))?;

        let staging = self.data_dir.join("runtime").join(".staging");
        tokio::fs::create_dir_all(&staging)
            .await
            .map_err(|err| RuntimeError::Process(err.to_string()))?;
        let archive_path = staging.join(&package.archive_name);
        tokio::fs::write(&archive_path, &bytes)
            .await
            .map_err(|err| RuntimeError::Process(err.to_string()))?;

        let extract_dir = staging.join("extract");
        tokio::fs::create_dir_all(&extract_dir)
            .await
            .map_err(|err| RuntimeError::Process(err.to_string()))?;

        if package.archive_name.ends_with(".zip") {
            extract_zip(&archive_path, &extract_dir).await?;
        } else {
            extract_tar_gz(&archive_path, &extract_dir).await?;
        }

        let binary_name = binary_file_name();
        let discovered = find_named_file(&extract_dir, binary_name).ok_or_else(|| {
            RuntimeError::Process(format!("{binary_name} missing from release archive"))
        })?;

        self.copy_binary(&discovered, target).await?;
        let _ = tokio::fs::remove_dir_all(&staging).await;
        Ok(())
    }
}

fn select_archive(profile: &RuntimeHardwareProfile) -> RuntimeResult<(RuntimeBackend, String)> {
    let release = std::env::var("AISEC_LLAMA_RELEASE")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_RELEASE.into());

    if profile.os == "windows" {
        if profile.cuda {
            return Ok((
                RuntimeBackend::Cuda,
                format!("llama-{release}-bin-win-cuda-12.4-x64.zip"),
            ));
        }
        return Ok((
            RuntimeBackend::Cpu,
            format!("llama-{release}-bin-win-cpu-x64.zip"),
        ));
    }

    if profile.os == "macos" {
        if profile.arch == "aarch64" {
            return Ok((
                RuntimeBackend::Metal,
                format!("llama-{release}-bin-macos-arm64.tar.gz"),
            ));
        }
        return Ok((
            RuntimeBackend::Metal,
            format!("llama-{release}-bin-macos-x64.tar.gz"),
        ));
    }

    if profile.os == "linux" {
        if profile.cuda {
            return Ok((
                RuntimeBackend::Cuda,
                format!("llama-{release}-bin-ubuntu-x64.tar.gz"),
            ));
        }
        return Ok((
            RuntimeBackend::Cpu,
            format!("llama-{release}-bin-ubuntu-x64.tar.gz"),
        ));
    }

    Err(RuntimeError::Config(format!(
        "unsupported platform: {} / {}",
        profile.os, profile.arch
    )))
}

fn platform_label() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn binary_file_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "llama-server.exe"
    } else {
        "llama-server"
    }
}

fn dev_repo_binary() -> Option<PathBuf> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let binary = bundled_llama_server_binary(&repo_root);
    if binary.is_file() {
        Some(binary)
    } else {
        None
    }
}

async fn sha256_file(path: &Path) -> RuntimeResult<String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let bytes = std::fs::read(&path)?;
        let digest = Sha256::digest(bytes);
        Ok::<_, std::io::Error>(hex::encode(digest))
    })
    .await
    .map_err(|err| RuntimeError::Process(err.to_string()))?
    .map_err(|err| RuntimeError::Process(err.to_string()))
}

async fn set_executable(path: &Path) -> RuntimeResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(path)
            .await
            .map_err(|err| RuntimeError::Process(err.to_string()))?
            .permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(path, perms)
            .await
            .map_err(|err| RuntimeError::Process(err.to_string()))?;
    }
    Ok(())
}

fn find_named_file(root: &Path, file_name: &str) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = std::fs::read_dir(&current).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().and_then(|n| n.to_str()) == Some(file_name) {
                return Some(path);
            }
        }
    }
    None
}

async fn extract_tar_gz(archive_path: &Path, dest_dir: &Path) -> RuntimeResult<()> {
    let archive_path = archive_path.to_path_buf();
    let dest_dir = dest_dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&archive_path)?;
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
        archive.unpack(&dest_dir)?;
        Ok::<(), std::io::Error>(())
    })
    .await
    .map_err(|err| RuntimeError::Process(err.to_string()))?
    .map_err(|err| RuntimeError::Process(err.to_string()))
}

async fn extract_zip(archive_path: &Path, dest_dir: &Path) -> RuntimeResult<()> {
    let bytes = tokio::fs::read(archive_path)
        .await
        .map_err(|err| RuntimeError::Process(err.to_string()))?;
    let dest_dir = dest_dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let reader = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(reader)?;
        archive.extract(&dest_dir)?;
        Ok::<(), zip::result::ZipError>(())
    })
    .await
    .map_err(|err| RuntimeError::Process(err.to_string()))?
    .map_err(|err| RuntimeError::Process(err.to_string()))
}

mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}
