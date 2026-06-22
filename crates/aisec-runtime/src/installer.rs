//! Download, verify, and install embedded llama-server binaries.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::info;

use crate::error::{RuntimeError, RuntimeResult};
use crate::hardware::RuntimeHardwareProfile;
use crate::manifest::{RuntimeBackend, RuntimeManifest};
use crate::paths::{bundled_llama_server_binary, bundled_runtime_dir};

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

    pub fn install_dir(&self) -> PathBuf {
        bundled_runtime_dir(&self.data_dir)
    }

    pub fn install_path(&self) -> PathBuf {
        bundled_llama_server_binary(&self.data_dir)
    }

    pub fn is_installed(&self) -> bool {
        self.install_path().is_file()
    }

    /// Smoke-test the installed binary (catches missing dylibs / truncated downloads).
    pub async fn validate_binary(path: &Path) -> RuntimeResult<()> {
        if let Some(dir) = path.parent().filter(|p| p.is_dir()) {
            let dir = dir.to_path_buf();
            tokio::task::spawn_blocking(move || ensure_dylib_symlinks(&dir))
                .await
                .map_err(|err| RuntimeError::Process(err.to_string()))?
                .map_err(|err| RuntimeError::Process(err.to_string()))?;
        }

        let path = path.to_path_buf();
        let output = tokio::task::spawn_blocking(move || {
            std::process::Command::new(&path)
                .arg("--version")
                .current_dir(
                    path.parent()
                        .filter(|p| p.is_dir())
                        .unwrap_or_else(|| Path::new(".")),
                )
                .output()
        })
        .await
        .map_err(|err| RuntimeError::Process(err.to_string()))?
        .map_err(|err| RuntimeError::Process(format!("failed to run llama-server: {err}")))?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(RuntimeError::Process(format!(
            "llama-server validation failed (exit {}): {}{}",
            output.status,
            stderr.trim(),
            if stdout.trim().is_empty() {
                String::new()
            } else {
                format!(" | {}", stdout.trim())
            }
        )))
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
        mut progress: impl FnMut(&str),
    ) -> RuntimeResult<RuntimeManifest> {
        progress("selecting runtime package");
        let package = Self::select_package(profile)?;
        let target = self.install_path();
        let valid = target.is_file() && Self::validate_binary(&target).await.is_ok();

        if valid {
            progress("runtime already installed");
        } else if let Some(bundle_dir) = self.bundled_runtime_dir() {
            progress("installing bundled runtime");
            self.install_bundle_dir(&bundle_dir).await?;
        } else if let Some(dev_dir) = dev_repo_runtime_dir() {
            progress("installing development runtime");
            self.install_bundle_dir(&dev_dir).await?;
        } else {
            progress("downloading runtime");
            self.download_and_extract(&package).await?;
        }

        progress("verifying runtime");
        ensure_dylib_symlinks(&self.install_dir())
            .map_err(|err| RuntimeError::Process(err.to_string()))?;
        Self::validate_binary(&target).await?;
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
        if Self::validate_binary(&manifest.install_path).await.is_err() {
            return Ok(false);
        }
        let actual = sha256_file(&manifest.install_path).await?;
        Ok(manifest.sha256.as_deref() == Some(actual.as_str()))
    }

    fn bundled_runtime_dir(&self) -> Option<PathBuf> {
        self.bundled_binary
            .as_ref()
            .and_then(|binary| binary.parent().map(Path::to_path_buf))
            .filter(|dir| bundled_llama_server_binary(dir).is_file())
    }

    async fn install_bundle_dir(&self, source_dir: &Path) -> RuntimeResult<()> {
        let install_dir = self.install_dir();
        tokio::fs::create_dir_all(&install_dir)
            .await
            .map_err(|err| RuntimeError::Process(err.to_string()))?;

        let source_dir = source_dir.to_path_buf();
        let install_dir = install_dir.clone();
        tokio::task::spawn_blocking(move || copy_runtime_tree(&source_dir, &install_dir))
            .await
            .map_err(|err| RuntimeError::Process(err.to_string()))?
            .map_err(|err| RuntimeError::Process(err.to_string()))
    }

    async fn download_and_extract(&self, package: &RuntimePackage) -> RuntimeResult<()> {
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

        let bundle_dir = discovered.parent().ok_or_else(|| {
            RuntimeError::Process("llama-server has no parent directory in archive".into())
        })?;
        self.install_bundle_dir(bundle_dir).await?;
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

fn dev_repo_runtime_dir() -> Option<PathBuf> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = bundled_runtime_dir(&repo_root);
    if bundled_llama_server_binary(&repo_root).is_file() {
        Some(dir)
    } else {
        None
    }
}

fn copy_runtime_tree(source_dir: &Path, install_dir: &Path) -> Result<(), std::io::Error> {
    let entries: Vec<_> = std::fs::read_dir(source_dir)?.collect::<Result<Vec<_>, _>>()?;

    for entry in &entries {
        let file_type = entry.file_type()?;
        if !file_type.is_file() {
            continue;
        }
        let dest = install_dir.join(entry.file_name());
        std::fs::copy(entry.path(), &dest)?;
        if entry.file_name() == binary_file_name() {
            set_executable_sync(&dest)?;
        }
    }

    for entry in &entries {
        if !entry.file_type()?.is_symlink() {
            continue;
        }
        let link_target = std::fs::read_link(entry.path())?;
        let dest = install_dir.join(entry.file_name());
        if dest.exists() {
            std::fs::remove_file(&dest)?;
        }
        symlink_file(&link_target, &dest)?;
    }

    ensure_dylib_symlinks(install_dir)?;
    Ok(())
}

/// Recreate versioned dylib symlinks when only real files were copied (e.g. partial bundles).
fn ensure_dylib_symlinks(install_dir: &Path) -> Result<(), std::io::Error> {
    let versioned_files: Vec<String> = std::fs::read_dir(install_dir)?
        .flatten()
        .filter(|entry| entry.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("lib") && name.ends_with(".dylib"))
        .collect();

    for file in versioned_files {
        let Some(link0) = dylib_major_link_name(&file) else {
            continue;
        };
        let link0_path = install_dir.join(&link0);
        if !link0_path.exists() {
            symlink_file(&file, &link0_path)?;
        }

        let base = link0.strip_suffix(".0.dylib").unwrap_or(link0.as_str());
        let link_path = install_dir.join(format!("{base}.dylib"));
        if !link_path.exists() {
            symlink_file(&link0, &link_path)?;
        }
    }

    Ok(())
}

fn dylib_major_link_name(versioned: &str) -> Option<String> {
    let name = versioned.strip_suffix(".dylib")?;
    let rest = name.strip_prefix("lib")?;
    let parts: Vec<&str> = rest.split('.').collect();
    if parts.len() < 3 {
        return None;
    }
    let base = parts[0];
    Some(format!("lib{base}.0.dylib"))
}

fn symlink_file(target: impl AsRef<Path>, link: impl AsRef<Path>) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        let _ = (target, link);
        Ok(())
    }
}

fn set_executable_sync(path: &Path) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms)?;
    }
    let _ = path;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dylib_major_link_names() {
        assert_eq!(
            dylib_major_link_name("libllama-common.0.0.9551.dylib").as_deref(),
            Some("libllama-common.0.dylib")
        );
        assert_eq!(
            dylib_major_link_name("libggml-base.0.13.1.dylib").as_deref(),
            Some("libggml-base.0.dylib")
        );
    }
}
