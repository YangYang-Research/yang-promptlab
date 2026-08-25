//! Remote-oriented AI runtime host — model provider, lifecycle, and stubs for legacy local APIs.

pub mod benchmark;
pub mod config;
pub mod discovery;
pub mod embedded;
pub mod error;
pub mod hardware;
pub mod inference_adapter;
pub mod launcher;
pub mod local_runtime_adapter;
pub mod logs;
pub mod manager;
pub mod monitor;
pub mod paths;
pub mod provider;
pub mod registry;
pub mod state;
pub mod supervisor;
pub mod watch;

pub use benchmark::RuntimeBenchmarkResult;
pub use config::RuntimeConfig;
pub use discovery::{check_health, discover_models, discover_models_in_dir, DiscoveredModel};
pub use embedded::{EmbeddedModelProvider, SharedModelProvider};
pub use hardware::RuntimeHardwareProfile;
pub use inference_adapter::ModelProviderRuntime;
pub use error::{RuntimeError, RuntimeResult};
pub use local_runtime_adapter::{
    GfxBackend, InferRequest, InferResponse, LocalRuntimeAdapter, LocalRuntimeCapabilities,
    RuntimeBackend,
};
pub use logs::RuntimeLogEntry;
pub use manager::{RuntimeManager, RuntimeStatusSnapshot};
pub use monitor::RuntimeHealthReport;
pub use paths::{models_dir, runtime_dir};
pub use provider::{ModelProvider, ModelProviderHealth};
pub use registry::{BuiltinModelRegistry, RegistryEntry, RegistryUpdateResult};
pub use state::RuntimeLifecycleState;
pub use supervisor::{RuntimeProcessState, RuntimeSupervisor};
