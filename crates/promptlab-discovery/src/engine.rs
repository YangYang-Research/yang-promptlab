use std::time::Instant;

use promptlab_core::PromptLabResult;
use async_trait::async_trait;
use tracing::{info, instrument, warn};

use crate::browser::{BrowserConfig, BrowserCrawler};
use crate::client::HttpClient;
use crate::config::{DiscoveryConfig, SessionAuthMaterial};
use crate::crawler::Crawler;
use crate::detectors::{probe_ai_paths, probe_graphql_paths, probe_openapi_paths};
use crate::types::{DiscoveryReport, DiscoveredEndpoint};
use crate::url_policy::{origin_of, validate_target_url};

/// Attack-surface discovery engine (MVP).
pub struct DiscoveryEngine {
    config: DiscoveryConfig,
    client: HttpClient,
    session_auth: Option<SessionAuthMaterial>,
}

impl DiscoveryEngine {
    pub fn new(config: DiscoveryConfig) -> PromptLabResult<Self> {
        let client = HttpClient::new(config.clone())?;
        Ok(Self {
            config,
            client,
            session_auth: None,
        })
    }

    /// Attach authenticated session cookies/tokens and optional Playwright storageState.
    pub fn with_session_auth(mut self, auth: SessionAuthMaterial) -> PromptLabResult<Self> {
        self.client = HttpClient::new(self.config.clone())?.with_auth_headers(auth.headers.clone());
        self.session_auth = Some(auth);
        Ok(self)
    }

    pub fn with_defaults() -> PromptLabResult<Self> {
        Self::new(DiscoveryConfig::default())
    }

    pub fn config(&self) -> &DiscoveryConfig {
        &self.config
    }

    /// Run full discovery against a seed URL.
    #[instrument(skip(self), fields(seed = %seed_url))]
    pub async fn discover(&self, seed_url: &str) -> PromptLabResult<DiscoveryReport> {
        if self.uses_browser_session() {
            let browser_config = self.browser_config_from_session();
            return self.discover_with_browser(seed_url, browser_config).await;
        }
        self.discover_http_only(seed_url).await
    }

    async fn discover_http_only(&self, seed_url: &str) -> PromptLabResult<DiscoveryReport> {
        let started = Instant::now();
        let seed = validate_target_url(seed_url, &self.config)?;
        let origin = origin_of(&seed);

        info!(%origin, "starting discovery");

        let mut endpoints = Vec::new();
        let mut errors = Vec::new();
        let mut probes_sent = 0usize;

        if self.config.probe_static_paths {
            let probe_results = self.run_static_probes(&origin).await;
            probes_sent = probe_results.probes_sent;
            endpoints.extend(probe_results.endpoints);
            errors.extend(probe_results.errors);
        }

        let crawler = Crawler::new(self.client.clone(), seed, self.config.clone());
        match crawler.run().await {
            Ok(output) => {
                endpoints.extend(output.endpoints);
                errors.extend(output.errors);

                let mut stats = output.stats;
                stats.probes_sent = probes_sent;
                stats.duration_ms = started.elapsed().as_millis() as u64;

                let endpoints = DiscoveryReport::dedupe_endpoints(endpoints);

                info!(
                    pages = stats.pages_fetched,
                    endpoints = endpoints.len(),
                    duration_ms = stats.duration_ms,
                    "discovery complete"
                );

                return Ok(DiscoveryReport {
                    seed_url: seed_url.to_string(),
                    origin,
                    endpoints,
                    stats,
                    errors,
                });
            }
            Err(err) => {
                errors.push(err.client_message());
            }
        }

        let endpoints = DiscoveryReport::dedupe_endpoints(endpoints);
        Ok(DiscoveryReport {
            seed_url: seed_url.to_string(),
            origin,
            endpoints,
            stats: crate::types::CrawlStats {
                pages_fetched: 0,
                pages_failed: 0,
                links_extracted: 0,
                probes_sent,
                duration_ms: started.elapsed().as_millis() as u64,
            },
            errors,
        })
    }

    /// Run standard HTTP discovery, then augment it with a real browser
    /// (Playwright/Chromium) pass that renders the SPA and captures network
    /// traffic. Captured API requests are merged into the report as endpoints.
    ///
    /// Browser failures (e.g. Node/Playwright not installed) are non-fatal: the
    /// HTTP discovery report is still returned with the failure recorded in
    /// `errors`, so callers degrade gracefully.
    #[instrument(skip(self, browser_config), fields(seed = %seed_url))]
    pub async fn discover_with_browser(
        &self,
        seed_url: &str,
        browser_config: BrowserConfig,
    ) -> PromptLabResult<DiscoveryReport> {
        // Validate up-front under the same SSRF policy as HTTP discovery.
        validate_target_url(seed_url, &self.config)?;

        let mut report = self.discover_http_only(seed_url).await?;

        let crawler = BrowserCrawler::new(browser_config);
        match crawler.capture(seed_url).await {
            Ok(capture) => {
                info!(
                    captured = capture.requests.len(),
                    exported = capture.endpoints.len(),
                    "browser capture merged into discovery report"
                );
                report.endpoints.extend(capture.endpoints);
                report.errors.extend(capture.errors);
                report.endpoints =
                    DiscoveryReport::dedupe_endpoints(std::mem::take(&mut report.endpoints));
            }
            Err(err) => {
                warn!(error = %err.client_message(), "browser capture failed; returning HTTP-only report");
                report
                    .errors
                    .push(format!("browser capture failed: {}", err.client_message()));
            }
        }

        let _ = crawler.close().await;
        Ok(report)
    }

    fn uses_browser_session(&self) -> bool {
        self.session_auth
            .as_ref()
            .and_then(|auth| auth.storage_state_path.as_ref())
            .is_some()
    }

    fn browser_config_from_session(&self) -> BrowserConfig {
        let mut browser_config = BrowserConfig::default();
        if let Some(auth) = &self.session_auth {
            browser_config.storage_state_path = auth.storage_state_path.clone();
        }
        browser_config
    }
}

struct ProbeOutput {
    endpoints: Vec<DiscoveredEndpoint>,
    probes_sent: usize,
    errors: Vec<String>,
}

impl DiscoveryEngine {
    async fn run_static_probes(&self, origin: &str) -> ProbeOutput {
        use crate::detectors::{ai_probe_paths, graphql_probe_paths, openapi_probe_paths};

        let mut endpoints = Vec::new();
        let errors = Vec::new();

        let openapi_urls = openapi_probe_paths(origin);
        let graphql_urls = graphql_probe_paths(origin);
        let ai_urls = ai_probe_paths(origin);
        let probes_sent = openapi_urls.len() + graphql_urls.len() + ai_urls.len();

        endpoints.extend(probe_openapi_paths(&self.client, origin).await);
        endpoints.extend(probe_graphql_paths(&self.client, origin).await);
        endpoints.extend(probe_ai_paths(&self.client, origin).await);

        ProbeOutput {
            endpoints,
            probes_sent,
            errors,
        }
    }
}

/// Trait aligned with PromptLab engine contract (`discover()` phase).
#[async_trait]
pub trait SurfaceDiscovery: Send + Sync {
    async fn discover(&self, seed_url: &str) -> PromptLabResult<DiscoveryReport>;
}

#[async_trait]
impl SurfaceDiscovery for DiscoveryEngine {
    async fn discover(&self, seed_url: &str) -> PromptLabResult<DiscoveryReport> {
        DiscoveryEngine::discover(self, seed_url).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_constructs() {
        DiscoveryEngine::with_defaults().expect("engine");
    }
}
