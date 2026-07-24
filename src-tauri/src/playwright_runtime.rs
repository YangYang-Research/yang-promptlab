//! Resolve bundled Playwright + Node paths for release, with dev fallbacks.

use std::path::{Path, PathBuf};

use promptlab_auth::AuthEngineConfig;
use promptlab_core::PromptLabResult;
use tauri::{AppHandle, Manager};
use tracing::info;

fn bundled_paths(resource_dir: &Path) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let root = resource_dir.join("playwright");
    let auth_dir = root.join("auth");
    let node_bin = if cfg!(windows) {
        root.join("node/node.exe")
    } else {
        root.join("node/bin/node")
    };
    let runner = auth_dir.join("runner.mjs");
    let browsers = auth_dir.join("browsers");
    (node_bin, runner, auth_dir, browsers)
}

fn bundled_is_complete(node_bin: &Path, runner: &Path, auth_dir: &Path) -> bool {
    node_bin.is_file()
        && runner.is_file()
        && auth_dir.join("node_modules/playwright/package.json").is_file()
}

fn dev_paths() -> Option<(PathBuf, PathBuf, PathBuf, PathBuf)> {
    let auth_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../crates/promptlab-auth/playwright");
    let runner = auth_dir.join("runner.mjs");
    if !runner.is_file() {
        return None;
    }

    let node_bin = PathBuf::from(if cfg!(windows) { "node.exe" } else { "node" });
    let browsers = auth_dir.join("browsers");
    Some((node_bin, runner, auth_dir, browsers))
}

pub fn resolve_auth_engine_config(app: &AppHandle) -> PromptLabResult<AuthEngineConfig> {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let (node_bin, runner, auth_dir, browsers) = bundled_paths(&resource_dir);
        if bundled_is_complete(&node_bin, &runner, &auth_dir) {
            info!(
                node = %node_bin.display(),
                runner = %runner.display(),
                browsers = %browsers.display(),
                "using bundled Playwright auth runtime"
            );
            return Ok(AuthEngineConfig::default().with_playwright_bundle(
                node_bin,
                runner,
                auth_dir,
                browsers,
            ));
        }
    }

    if let Some((node_bin, runner, auth_dir, browsers)) = dev_paths() {
        info!(
            runner = %runner.display(),
            "using development Playwright auth runtime (system Node.js)"
        );
        let mut config = AuthEngineConfig::default();
        config.node_bin = node_bin.to_string_lossy().into_owned();
        config.playwright_runner = Some(runner);
        config.runner_workdir = Some(auth_dir);
        if browsers.is_dir() {
            config.playwright_browsers_path = Some(browsers);
        }
        return Ok(config);
    }

    info!("Playwright auth runtime not found; browser recording unavailable until bundled");
    Ok(AuthEngineConfig::default())
}
