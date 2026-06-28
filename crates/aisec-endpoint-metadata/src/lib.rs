//! AI Endpoint Metadata — single source of truth produced during Discovery.
//!
//! Downstream modules (Planner, Payload Generator, Attack Engine) consume only
//! persisted [`AiEndpointMetadata`]; they must not re-fingerprint or re-infer schemas.

pub mod capability;
pub mod classify;
pub mod pipeline;
pub mod risk;
pub mod schema;
pub mod types;

pub use capability::CapabilityDetector;
pub use classify::EndpointClassifier;
pub use pipeline::{
    analyze_endpoint, analyze_endpoints_batch, body_template_from_metadata, build_payload_body,
    DiscoveryAnalysisInput,
};
pub use risk::RiskScorer;
pub use schema::SchemaInferenceEngine;
pub use types::*;
