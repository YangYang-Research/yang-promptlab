//! Unified AI inference gateway for AISec.
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
pub use types::*;
