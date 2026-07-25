//! Proxy settings commands — configure and test outbound HTTP(S)/SOCKS proxy.

use promptlab_core::{
    build_http_client_with, current_proxy_settings, install_proxy_settings, load_proxy_settings,
    save_proxy_settings, HttpClientOptions, ProxySettings, DEFAULT_PROXY_TEST_URL,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxySettingsDto {
    pub enabled: bool,
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub no_proxy: String,
    pub test_url: String,
    #[serde(default)]
    pub allow_insecure_tls: bool,
}

impl From<ProxySettings> for ProxySettingsDto {
    fn from(value: ProxySettings) -> Self {
        Self {
            enabled: value.enabled,
            url: value.url,
            username: value.username,
            password: value.password,
            no_proxy: value.no_proxy,
            test_url: value.test_url,
            allow_insecure_tls: value.allow_insecure_tls,
        }
    }
}

impl From<ProxySettingsDto> for ProxySettings {
    fn from(value: ProxySettingsDto) -> Self {
        Self {
            enabled: value.enabled,
            url: value.url.trim().to_string(),
            username: value
                .username
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            password: value.password.filter(|s| !s.is_empty()),
            no_proxy: value.no_proxy,
            test_url: {
                let trimmed = value.test_url.trim().to_string();
                if trimmed.is_empty() {
                    DEFAULT_PROXY_TEST_URL.into()
                } else {
                    trimmed
                }
            },
            allow_insecure_tls: value.allow_insecure_tls,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyTestResultDto {
    pub ok: bool,
    pub latency_ms: u64,
    pub status: Option<u16>,
    pub message: String,
    pub via_proxy: bool,
}

#[tauri::command]
pub async fn proxy_get(state: State<'_, AppState>) -> CommandResult<ProxySettingsDto> {
    let settings = load_proxy_settings(&state.environment().config)
        .map_err(CommandError::from)?;
    install_proxy_settings(settings.clone());
    Ok(settings.into())
}

#[tauri::command]
pub async fn proxy_set(
    state: State<'_, AppState>,
    settings: ProxySettingsDto,
) -> CommandResult<ProxySettingsDto> {
    let settings: ProxySettings = settings.into();
    settings.validate().map_err(CommandError::from)?;
    save_proxy_settings(&state.environment().config, &settings).map_err(CommandError::from)?;
    install_proxy_settings(settings.clone());
    Ok(settings.into())
}

#[tauri::command]
pub async fn proxy_test_connection(
    settings: ProxySettingsDto,
) -> CommandResult<ProxyTestResultDto> {
    let settings: ProxySettings = settings.into();
    if settings.enabled {
        settings.validate().map_err(CommandError::from)?;
    }

    let test_url = settings.normalized_test_url().to_string();
    let via_proxy = settings.enabled;
    let started = std::time::Instant::now();

    match send_proxy_test(&settings, &test_url).await {
        Ok(status) => Ok(success_result(status, started.elapsed().as_millis() as u64, via_proxy, &test_url)),
        Err(err) => {
            // HTTPS through MITM proxies fails cert verify unless allow_insecure_tls is on.
            if settings.enabled
                && !settings.allow_insecure_tls
                && test_url.to_ascii_lowercase().starts_with("https://")
            {
                let mut insecure = settings.clone();
                insecure.allow_insecure_tls = true;
                if let Ok(status) = send_proxy_test(&insecure, &test_url).await {
                    return Ok(ProxyTestResultDto {
                        ok: false,
                        latency_ms: started.elapsed().as_millis() as u64,
                        status: Some(status),
                        message: format!(
                            "Proxy {proxy} reached {test_url} (HTTP {status}), but TLS verify failed. \
                             Enable “Allow insecure TLS” for MITM proxies (Charles / mitmproxy / Burp).",
                            proxy = settings.url.trim(),
                        ),
                        via_proxy: true,
                    });
                }
            }

            Ok(ProxyTestResultDto {
                ok: false,
                latency_ms: started.elapsed().as_millis() as u64,
                status: None,
                message: format_proxy_failure(&settings, &test_url, &err),
                via_proxy,
            })
        }
    }
}

async fn send_proxy_test(settings: &ProxySettings, test_url: &str) -> Result<u16, String> {
    let client = build_http_client_with(
        settings,
        HttpClientOptions::default()
            .with_timeout(std::time::Duration::from_secs(20))
            .with_connect_timeout(std::time::Duration::from_secs(8))
            .with_redirect_limit(5),
    )
    .map_err(|e| e.to_string())?;
    let response = client
        .get(test_url)
        .send()
        .await
        .map_err(|e| error_chain(&e))?;
    Ok(response.status().as_u16())
}

fn success_result(status: u16, latency_ms: u64, via_proxy: bool, test_url: &str) -> ProxyTestResultDto {
    let ok = (200..300).contains(&status) || status == 204;
    ProxyTestResultDto {
        ok,
        latency_ms,
        status: Some(status),
        message: if ok {
            if via_proxy {
                format!("Proxy reachable — HTTP {status} from {test_url}")
            } else {
                format!("Direct connection OK — HTTP {status} from {test_url}")
            }
        } else {
            format!("Unexpected HTTP {status} from {test_url}")
        },
        via_proxy,
    }
}

fn format_proxy_failure(settings: &ProxySettings, test_url: &str, err: &str) -> String {
    if settings.enabled {
        format!(
            "Connection failed via proxy {} to {test_url}: {err}",
            settings.url.trim()
        )
    } else {
        format!("Connection failed to {test_url}: {err}")
    }
}

fn error_chain(err: &dyn std::error::Error) -> String {
    let mut parts = vec![err.to_string()];
    let mut current = err.source();
    while let Some(source) = current {
        parts.push(source.to_string());
        current = source.source();
    }
    parts.join(" → ")
}

/// Expose current in-memory settings (for diagnostics).
#[allow(dead_code)]
pub fn runtime_proxy_snapshot() -> ProxySettings {
    current_proxy_settings()
}
