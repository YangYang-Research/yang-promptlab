//! Unified AI inference gateway for PromptLab.
//!
//! All AI capabilities (judge, planner, generator, reports, future modules)
//! must obtain inference exclusively through [`AiInferenceGateway`] / [`GatewaySession`].

pub mod capabilities;
pub mod config;
pub mod error;
pub mod gateway;
pub mod manager;
pub mod prompts;
pub mod provider;
pub mod runtime;
pub mod traffic;
pub mod types;

pub use capabilities::ModelCapabilities;
pub use config::{
    AiRuntimeConfiguration, InferenceMode, InferenceProvider, RuntimeHealth, config_path,
    load_config, save_config,
};
pub use error::{InferenceError, InferenceResult};
pub use gateway::{
    AiInferenceGateway, DefaultAiInferenceGateway, GatewayLlmBridge, GatewaySession,
    InferenceSession,
};
pub use manager::InferenceRuntimeManager;
pub use prompts::{PromptBuilder, PromptComposer, PromptContext, PromptId, PromptRegistry, PromptTemplate};
pub use provider::{LlamaCppAdapter, ProviderAdapter, RemoteAdapterSettings, RemoteProviderAdapter};
pub use runtime::{LocalRuntimeAdapterBridge, RuntimeAdapter};
pub use traffic::{
    TrafficBucket, TrafficDirection, TrafficEvent, TrafficSnapshot,
    drain_pending as traffic_drain_pending, ensure_started as traffic_ensure_started,
    lifetime_totals as traffic_lifetime_totals, record_received, record_roundtrip, record_sent,
    set_lifetime_totals as traffic_set_lifetime_totals, snapshot as traffic_snapshot,
    snapshot_from_events as traffic_snapshot_from_events,
};
pub use types::*;
