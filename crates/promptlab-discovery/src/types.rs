use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Kind of discovered endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointKind {
    Link,
    RestApi,
    OpenApi,
    GraphQl,
    AiEndpoint,
    /// An HTML `<form>` submission target.
    Form,
    /// A referenced JavaScript file.
    #[serde(rename = "javascript")]
    JavaScript,
}

impl EndpointKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Link => "link",
            Self::RestApi => "rest_api",
            Self::OpenApi => "openapi",
            Self::GraphQl => "graphql",
            Self::AiEndpoint => "ai_endpoint",
            Self::Form => "form",
            Self::JavaScript => "javascript",
        }
    }
}

/// A discovered URL with classification metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoveredEndpoint {
    pub url: String,
    pub kind: EndpointKind,
    pub method: Option<String>,
    pub confidence: f32,
    pub evidence: String,
    pub source_url: Option<String>,
    pub discovered_at: OffsetDateTime,
}

impl DiscoveredEndpoint {
    pub fn new(
        url: impl Into<String>,
        kind: EndpointKind,
        confidence: f32,
        evidence: impl Into<String>,
    ) -> Self {
        Self {
            url: url.into(),
            kind,
            method: None,
            confidence,
            evidence: evidence.into(),
            source_url: None,
            discovered_at: OffsetDateTime::now_utc(),
        }
    }

    pub fn with_method(mut self, method: impl Into<String>) -> Self {
        self.method = Some(method.into());
        self
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source_url = Some(source.into());
        self
    }
}

/// Crawl statistics for observability.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CrawlStats {
    pub pages_fetched: usize,
    pub pages_failed: usize,
    pub links_extracted: usize,
    pub probes_sent: usize,
    pub duration_ms: u64,
}

/// Aggregated discovery report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoveryReport {
    pub seed_url: String,
    pub origin: String,
    pub endpoints: Vec<DiscoveredEndpoint>,
    pub stats: CrawlStats,
    pub errors: Vec<String>,
}

impl DiscoveryReport {
    pub fn endpoints_by_kind(&self, kind: EndpointKind) -> Vec<&DiscoveredEndpoint> {
        self.endpoints
            .iter()
            .filter(|ep| ep.kind == kind)
            .collect()
    }

    pub fn dedupe_endpoints(endpoints: Vec<DiscoveredEndpoint>) -> Vec<DiscoveredEndpoint> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for ep in endpoints {
            let key = (ep.url.clone(), ep.kind, ep.method.clone());
            if seen.insert(key) {
                out.push(ep);
            }
        }
        out.sort_by(|a, b| a.url.cmp(&b.url));
        out
    }
}

/// Internal crawl task queued for workers.
#[derive(Debug, Clone)]
pub(crate) struct CrawlTask {
    pub url: String,
    pub depth: u32,
    pub referrer: Option<String>,
}

/// HTTP response snapshot used by detectors.
#[derive(Debug, Clone)]
pub struct HttpSnapshot {
    pub url: String,
    pub status: u16,
    pub content_type: Option<String>,
    pub body: String,
}
