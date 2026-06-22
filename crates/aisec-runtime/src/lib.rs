//! Embedded local AI runtime supervisor and offline model registry.

pub mod benchmark;
pub mod config;
pub mod discovery;
pub mod embedded;
pub mod error;
pub mod hardware;
pub mod inference_adapter;
pub mod installer;
pub mod launcher;
pub mod logs;
pub mod manager;
pub mod manifest;
pub mod monitor;
pub mod paths;
pub mod provider;
pub mod registry;
pub mod runtime;
pub mod state;
pub mod supervisor;
pub mod watch;

pub use benchmark::RuntimeBenchmarkResult;
pub use config::{default_llama_base_url, RuntimeConfig};
#[allow(deprecated)]
pub use config::default_ollama_base_url;
pub use discovery::{check_health, discover_models, discover_models_in_dir, DiscoveredModel};
pub use embedded::{EmbeddedModelProvider, SharedModelProvider};
pub use hardware::RuntimeHardwareProfile;
pub use inference_adapter::ModelProviderRuntime;
pub use error::{RuntimeError, RuntimeResult};
pub use logs::RuntimeLogEntry;
pub use manager::{RuntimeManager, RuntimeStatusSnapshot};
pub use manifest::{RuntimeBackend, RuntimeManifest};
pub use monitor::RuntimeHealthReport;
pub use paths::{
    bundled_llama_server_binary, bundled_ollama_binary, bundled_runtime_dir, models_dir,
};
pub use provider::{ModelProvider, ModelProviderHealth};
pub use registry::{BuiltinModelRegistry, RegistryEntry, RegistryUpdateResult};
pub use runtime::{
    detect_quantization, validate_gguf_model, GgufQuantization, InferRequest, InferResponse,
    LlamaCppRuntime, LlamaCppRuntimeConfig,
};
pub use state::RuntimeLifecycleState;
pub use supervisor::{RuntimeProcessState, RuntimeSupervisor};
pub use watch::run_supervisor_watch;
