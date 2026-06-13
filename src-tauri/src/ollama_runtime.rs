//! Resolve bundled Ollama binary paths for release, with dev and system fallbacks.

use std::path::{Path, PathBuf};

use aisec_core::AisecResult;
use aisec_runtime::{bundled_ollama_binary, bundled_runtime_dir, RuntimeConfig};
use tauri::{AppHandle, Manager};
use tracing::info;

fn bundled_binary(resource_dir: &Path) -> PathBuf {
    bundled_ollama_binary(resource_dir)
}

fn dev_binary() -> Option<PathBuf> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let binary = bundled_ollama_binary(&repo_root);
    if binary.is_file() {
        return Some(binary);
    }
    None
}

fn system_ollama_binary() -> Option<PathBuf> {
    let name = if cfg!(windows) { "ollama.exe" } else { "ollama" };
    which::which(name).ok()
}

/// Resolve the best available Ollama binary and runtime configuration.
pub fn resolve_runtime_config(app: &AppHandle, data_dir: &Path) -> RuntimeConfig {
    let mut config = RuntimeConfig::new("", data_dir);

    if let Ok(resource_dir) = app.path().resource_dir() {
        let bundled = bundled_binary(&resource_dir);
        if bundled.is_file() {
            info!(
                binary = %bundled.display(),
                "using bundled embedded Ollama runtime"
            );
            return RuntimeConfig::new(&resource_dir, data_dir).with_binary(bundled);
        }
        config = RuntimeConfig::new(&resource_dir, data_dir);
    }

    if let Some(dev) = dev_binary() {
        info!(
            binary = %dev.display(),
            "using development embedded Ollama runtime from repo runtime/"
        );
        return config.with_binary(dev);
    }

    if let Some(system) = system_ollama_binary() {
        info!(
            binary = %system.display(),
            "using system Ollama binary (fallback)"
        );
        return config.with_binary(system);
    }

    info!(
        expected = %bundled_runtime_dir("").join(if cfg!(windows) {
            "ollama.exe"
        } else {
            "ollama"
        })
        .display(),
        "embedded Ollama runtime not found; local model install requires runtime/ollama"
    );
    config
}

pub async fn start_embedded_runtime(
    config: RuntimeConfig,
) -> AisecResult<(aisec_runtime::RuntimeSupervisor, bool)> {
    let mut supervisor = aisec_runtime::RuntimeSupervisor::with_config(config);
    match supervisor.ensure_running().await {
        Ok(()) => Ok((supervisor, true)),
        Err(aisec_runtime::RuntimeError::Unavailable) => {
            info!("embedded runtime binary unavailable; skipping auto-start");
            Ok((supervisor, false))
        }
        Err(err) => Err(aisec_core::AisecError::internal(err.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aisec_runtime::bundled_ollama_binary;

    #[test]
    fn dev_binary_path_is_under_repo_runtime() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
        let expected = bundled_ollama_binary(&repo_root);
        assert!(expected.ends_with(if cfg!(windows) { "ollama.exe" } else { "ollama" }));
    }
}
