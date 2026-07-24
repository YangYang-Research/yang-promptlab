//! Endpoint detectors for OpenAPI, GraphQL, REST APIs, and AI services.

mod ai;
mod api;
mod graphql;
mod openapi;
mod paths;

pub use ai::detect_ai_from_snapshot;
pub use ai::probe_ai_paths;
pub use api::detect_api_from_snapshot;
pub use graphql::detect_graphql_from_snapshot;
pub use graphql::probe_graphql_paths;
pub use openapi::detect_openapi_from_snapshot;
pub use openapi::probe_openapi_paths;
pub use paths::{ai_probe_paths, graphql_probe_paths, openapi_probe_paths};

use crate::types::{DiscoveredEndpoint, HttpSnapshot};

/// Run all content-based detectors against an HTTP snapshot.
pub fn detect_from_snapshot(snapshot: &HttpSnapshot, source_url: Option<&str>) -> Vec<DiscoveredEndpoint> {
    let mut found = Vec::new();
    found.extend(detect_openapi_from_snapshot(snapshot, source_url));
    found.extend(detect_graphql_from_snapshot(snapshot, source_url));
    found.extend(detect_api_from_snapshot(snapshot, source_url));
    found.extend(detect_ai_from_snapshot(snapshot, source_url));
    found
}
