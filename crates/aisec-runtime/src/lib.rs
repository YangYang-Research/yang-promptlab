//! Embedded local AI runtime supervisor and offline model registry.

pub mod embedded;
pub mod error;
pub mod paths;
pub mod provider;
pub mod registry;
pub mod supervisor;

pub use embedded::{EmbeddedModelProvider, SharedModelProvider};
pub use error::{RuntimeError, RuntimeResult};
pub use paths::{bundled_runtime_dir, models_dir};
pub use provider::{ModelProvider, ModelProviderHealth};
pub use registry::{BuiltinModelRegistry, RegistryEntry, RegistryUpdateResult};
pub use supervisor::{RuntimeProcessState, RuntimeSupervisor};
