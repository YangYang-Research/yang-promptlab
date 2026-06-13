//! Embedded local AI runtime supervisor and offline model registry.

pub mod config;
pub mod discovery;
pub mod embedded;
pub mod error;
pub mod inference_adapter;
pub mod paths;
pub mod provider;
pub mod registry;
pub mod supervisor;
pub mod watch;

pub use config::{default_ollama_base_url, RuntimeConfig};
pub use discovery::{check_health, discover_models, DiscoveredModel};
pub use embedded::{EmbeddedModelProvider, SharedModelProvider};
pub use inference_adapter::ModelProviderRuntime;
pub use error::{RuntimeError, RuntimeResult};
pub use paths::{bundled_ollama_binary, bundled_runtime_dir, models_dir};
pub use provider::{ModelProvider, ModelProviderHealth};
pub use registry::{BuiltinModelRegistry, RegistryEntry, RegistryUpdateResult};
pub use supervisor::{RuntimeProcessState, RuntimeSupervisor};
pub use watch::run_supervisor_watch;
