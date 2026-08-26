//! Remote-oriented AI runtime host — model provider, lifecycle, and remote-only stubs.

pub mod config;
pub mod embedded;
pub mod error;
pub mod hardware;
pub mod inference_adapter;
pub mod logs;
pub mod manager;
pub mod paths;
pub mod provider;
pub mod state;
pub mod supervisor;

pub use config::RuntimeConfig;
pub use embedded::{EmbeddedModelProvider, SharedModelProvider};
pub use error::{RuntimeError, RuntimeResult};
pub use hardware::RuntimeHardwareProfile;
pub use inference_adapter::ModelProviderRuntime;
pub use logs::RuntimeLogEntry;
pub use manager::{RuntimeHealthReport, RuntimeManager, RuntimeStatusSnapshot};
pub use paths::{models_dir, runtime_dir};
pub use provider::{ModelProvider, ModelProviderHealth};
pub use state::RuntimeLifecycleState;
pub use supervisor::{RuntimeProcessState, RuntimeSupervisor};
