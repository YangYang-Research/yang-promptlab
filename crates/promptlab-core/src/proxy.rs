//! Global HTTP(S)/SOCKS proxy settings for all outbound PromptLab traffic.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{RwLock, OnceLock};

use serde::{Deserialize, Serialize};

use crate::error::{PromptLabError, PromptLabResult};

pub const PROXY_SETTINGS_FILE: &str = "proxy_settings.json";
pub const DEFAULT_PROXY_TEST_URL: &str = "https://www.google.com/generate_204";

static PROXY_SETTINGS: OnceLock<RwLock<ProxySettings>> = OnceLock::new();

fn settings_lock() -> &'static RwLock<ProxySettings> {
    PROXY_SETTINGS.get_or_init(|| RwLock::new(ProxySettings::default()))
}

/// Persisted / runtime proxy configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProxySettings {
    /// When true, all outbound HTTP clients must use [`url`].
    pub enabled: bool,
    /// Proxy URL, e.g. `http://127.0.0.1:7890`, `socks5://127.0.0.1:1080`.
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// Comma-separated hosts that bypass the proxy (exact or suffix match).
    #[serde(default)]
    pub no_proxy: String,
    /// URL used by Settings → Test Connection.
    #[serde(default = "default_test_url")]
    pub test_url: String,
    /// Accept invalid / MITM proxy TLS certificates (Charles, mitmproxy, etc.).
    #[serde(default)]
    pub allow_insecure_tls: bool,
}

fn default_test_url() -> String {
    DEFAULT_PROXY_TEST_URL.into()
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            url: String::new(),
            username: None,
            password: None,
            no_proxy: "localhost,127.0.0.1".into(),
            test_url: default_test_url(),
            allow_insecure_tls: false,
        }
    }
}

impl ProxySettings {
    pub fn validate(&self) -> PromptLabResult<()> {
        if !self.enabled {
            return Ok(());
        }
        let url = self.url.trim();
        if url.is_empty() {
            return Err(PromptLabError::config(
                "proxy is enabled but proxy URL is empty",
            ));
        }
        let parsed = url::Url::parse(url)
            .map_err(|e| PromptLabError::config(format!("invalid proxy URL: {e}")))?;
        match parsed.scheme() {
            "http" | "https" | "socks4" | "socks4a" | "socks5" | "socks5h" => {}
            other => {
                return Err(PromptLabError::config(format!(
                    "unsupported proxy scheme '{other}' (use http, https, socks4, socks4a, socks5, or socks5h)"
                )));
            }
        }
        if parsed.host_str().is_none() {
            return Err(PromptLabError::config("proxy URL must include a host"));
        }
        Ok(())
    }

    pub fn normalized_test_url(&self) -> &str {
        let trimmed = self.test_url.trim();
        if trimmed.is_empty() {
            DEFAULT_PROXY_TEST_URL
        } else {
            trimmed
        }
    }

    pub fn no_proxy_hosts(&self) -> Vec<String> {
        self.no_proxy
            .split(',')
            .map(|s| s.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

pub fn proxy_settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join(PROXY_SETTINGS_FILE)
}

pub fn load_proxy_settings(config_dir: &Path) -> PromptLabResult<ProxySettings> {
    let path = proxy_settings_path(config_dir);
    if !path.exists() {
        return Ok(ProxySettings::default());
    }
    let raw = fs::read_to_string(&path).map_err(|e| {
        PromptLabError::internal(format!("failed to read {}: {e}", path.display()))
    })?;
    if raw.trim().is_empty() {
        return Ok(ProxySettings::default());
    }
    serde_json::from_str(&raw).map_err(|e| {
        PromptLabError::config(format!("invalid proxy settings JSON: {e}"))
    })
}

pub fn save_proxy_settings(config_dir: &Path, settings: &ProxySettings) -> PromptLabResult<()> {
    settings.validate()?;
    fs::create_dir_all(config_dir).map_err(|e| {
        PromptLabError::internal(format!("failed to create config dir: {e}"))
    })?;
    let path = proxy_settings_path(config_dir);
    let tmp = path.with_extension("json.tmp");
    let raw = serde_json::to_string_pretty(settings).map_err(|e| {
        PromptLabError::internal(format!("failed to serialize proxy settings: {e}"))
    })?;
    fs::write(&tmp, raw).map_err(|e| {
        PromptLabError::internal(format!("failed to write {}: {e}", tmp.display()))
    })?;
    fs::rename(&tmp, &path).map_err(|e| {
        PromptLabError::internal(format!("failed to persist proxy settings: {e}"))
    })?;
    Ok(())
}

/// Install settings into the process-global registry (call on startup / after save).
pub fn install_proxy_settings(settings: ProxySettings) {
    if let Ok(mut guard) = settings_lock().write() {
        *guard = settings;
    }
}

/// Current process-global proxy settings.
pub fn current_proxy_settings() -> ProxySettings {
    settings_lock()
        .read()
        .map(|g| g.clone())
        .unwrap_or_default()
}

/// Load from disk and install into the process-global registry.
pub fn bootstrap_proxy_settings(config_dir: &Path) -> PromptLabResult<ProxySettings> {
    let settings = load_proxy_settings(config_dir)?;
    install_proxy_settings(settings.clone());
    Ok(settings)
}
