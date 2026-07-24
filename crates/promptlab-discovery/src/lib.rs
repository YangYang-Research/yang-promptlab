//! PromptLab Discovery Engine — attack-surface enumeration for web targets.
//!
//! Crawls websites, extracts links, and detects REST APIs, OpenAPI specs,
//! GraphQL endpoints, and AI/LLM service routes.

pub mod browser;
pub mod client;
pub mod config;
pub mod crawler;
pub mod detectors;
pub mod engine;
pub mod extract;
pub mod retry;
pub mod types;
pub mod url_policy;

pub use browser::{BrowserCapture, BrowserConfig, BrowserCrawler, CapturedRequest, WaitUntil};
pub use client::HttpClient;
pub use config::{DiscoveryConfig, RetryConfig, SessionAuthMaterial};
pub use engine::{DiscoveryEngine, SurfaceDiscovery};
pub use types::{
    CrawlStats, DiscoveredEndpoint, DiscoveryReport, EndpointKind, HttpSnapshot,
};
