use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Discovery engine configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// Maximum crawl depth from the seed URL (0 = seed only).
    pub max_depth: u32,
    /// Maximum number of pages to fetch during crawl.
    pub max_pages: usize,
    /// Number of concurrent worker tasks.
    pub worker_count: usize,
    /// Per-request timeout.
    pub request_timeout: Duration,
    /// Retry policy for transient HTTP failures.
    pub retry: RetryConfig,
    /// Restrict crawling to the seed URL origin.
    pub same_origin_only: bool,
    /// Allow crawling private/loopback addresses (disabled by default).
    pub allow_private_network: bool,
    /// HTTP User-Agent header.
    pub user_agent: String,
    /// Whether to run static path probes (OpenAPI, GraphQL, AI paths).
    pub probe_static_paths: bool,
    /// Maximum response body bytes to read per request.
    pub max_body_bytes: usize,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_pages: 200,
            worker_count: 8,
            request_timeout: Duration::from_secs(15),
            retry: RetryConfig::default(),
            same_origin_only: true,
            allow_private_network: false,
            user_agent: format!("AISec-Discovery/{}", env!("CARGO_PKG_VERSION")),
            probe_static_paths: true,
            max_body_bytes: 2 * 1024 * 1024,
        }
    }
}

/// Exponential backoff retry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(250),
            max_delay: Duration::from_secs(5),
            multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }
        let factor = self.multiplier.powi(attempt as i32 - 1);
        let millis = (self.initial_delay.as_millis() as f64 * factor) as u64;
        Duration::from_millis(millis.min(self.max_delay.as_millis() as u64))
    }
}

/// Authenticated discovery material (cookies/tokens as HTTP headers + Playwright storageState).
#[derive(Debug, Clone, Default)]
pub struct SessionAuthMaterial {
    pub headers: HashMap<String, String>,
    pub storage_state_path: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_backoff_caps_at_max_delay() {
        let cfg = RetryConfig {
            max_attempts: 5,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(3),
            multiplier: 2.0,
        };
        assert_eq!(cfg.delay_for_attempt(1), Duration::from_secs(1));
        assert_eq!(cfg.delay_for_attempt(3), Duration::from_secs(3));
        assert_eq!(cfg.delay_for_attempt(10), Duration::from_secs(3));
    }
}
