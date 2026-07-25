//! Shared reqwest client builder that honors global [`crate::proxy::ProxySettings`].

use std::time::Duration;

use reqwest::redirect::Policy;
use reqwest::{Client, ClientBuilder, Proxy};
use url::Url;

use crate::error::{PromptLabError, PromptLabResult};
use crate::proxy::{current_proxy_settings, ProxySettings};

/// Optional knobs layered on top of proxy routing.
#[derive(Debug, Clone, Default)]
pub struct HttpClientOptions {
    pub timeout: Option<Duration>,
    pub connect_timeout: Option<Duration>,
    pub user_agent: Option<String>,
    /// `None` = reqwest default redirects; `Some(n)` = limited(n).
    pub redirect_limit: Option<usize>,
    pub no_gzip: bool,
}

impl HttpClientOptions {
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    pub fn with_user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = Some(ua.into());
        self
    }

    pub fn with_redirect_limit(mut self, limit: usize) -> Self {
        self.redirect_limit = Some(limit);
        self
    }

    pub fn without_gzip(mut self) -> Self {
        self.no_gzip = true;
        self
    }
}

/// Apply proxy (or force direct) onto a [`ClientBuilder`].
pub fn apply_proxy_settings(
    mut builder: ClientBuilder,
    settings: &ProxySettings,
) -> PromptLabResult<ClientBuilder> {
    if !settings.enabled {
        // Ignore process env proxies when user disabled proxy in Settings.
        return Ok(builder.no_proxy());
    }

    settings.validate()?;
    let proxy_url = settings.url.trim().to_string();
    let username = settings
        .username
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let password = settings.password.clone().unwrap_or_default();
    let no_proxy = settings.no_proxy_hosts();

    let proxy = if no_proxy.is_empty() {
        let mut proxy = Proxy::all(&proxy_url)
            .map_err(|e| PromptLabError::config(format!("invalid proxy: {e}")))?;
        if let Some(user) = username.as_deref() {
            proxy = proxy.basic_auth(user, &password);
        }
        proxy
    } else {
        let proxy_url_for_custom = proxy_url.clone();
        let username_for_custom = username.clone();
        let password_for_custom = password.clone();
        Proxy::custom(move |url| {
            let host = url.host_str().unwrap_or("").to_ascii_lowercase();
            if no_proxy.iter().any(|np| host_matches(&host, np)) {
                return None;
            }
            let mut parsed = Url::parse(&proxy_url_for_custom).ok()?;
            if let Some(user) = username_for_custom.as_deref() {
                let _ = parsed.set_username(user);
                let _ = parsed.set_password(Some(&password_for_custom));
            }
            Some(parsed)
        })
    };

    Ok(builder.proxy(proxy))
}

fn host_matches(host: &str, pattern: &str) -> bool {
    host == pattern || host.ends_with(&format!(".{pattern}"))
}

fn configure_builder(mut builder: ClientBuilder, opts: &HttpClientOptions) -> ClientBuilder {
    if let Some(timeout) = opts.timeout {
        builder = builder.timeout(timeout);
    }
    if let Some(timeout) = opts.connect_timeout {
        builder = builder.connect_timeout(timeout);
    }
    if let Some(ua) = &opts.user_agent {
        builder = builder.user_agent(ua.clone());
    }
    if let Some(limit) = opts.redirect_limit {
        builder = builder.redirect(Policy::limited(limit));
    }
    if opts.no_gzip {
        builder = builder.no_gzip();
    }
    builder
}

/// Build a client using explicit proxy settings (Settings test / draft preview).
pub fn build_http_client_with(
    settings: &ProxySettings,
    opts: HttpClientOptions,
) -> PromptLabResult<Client> {
    let mut builder = configure_builder(Client::builder(), &opts);
    if settings.enabled && settings.allow_insecure_tls {
        // Required for HTTPS through MITM proxies (self-signed / custom CA).
        builder = builder.danger_accept_invalid_certs(true);
    }
    apply_proxy_settings(builder, settings)?
        .build()
        .map_err(|e| PromptLabError::config(format!("failed to build HTTP client: {e}")))
}

/// Build a client using the process-global proxy settings.
pub fn build_http_client(opts: HttpClientOptions) -> PromptLabResult<Client> {
    build_http_client_with(&current_proxy_settings(), opts)
}

/// Convenience: default options, global proxy.
pub fn default_http_client() -> PromptLabResult<Client> {
    build_http_client(HttpClientOptions::default())
}
