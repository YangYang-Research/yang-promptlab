//! PromptLab model registry and vault helpers.
//!
//! Remote/third-party provider registry and Ollama-over-HTTP smoke tests.
//! Local embedded inference is removed — Remote + Ollama HTTP only.

pub mod error;
pub mod hardware;
pub mod manager;
pub mod registry;
pub mod runtime;
pub mod types;
pub mod verify;

pub use error::{ModelError, ModelResult};
pub use hardware::detect_hardware;
pub use manager::LocalModelManager;
pub use registry::{remote_entry_id, ModelRegistry};
pub use runtime::{
    InferenceRuntime, LocalInferenceEngine, MockInferenceRuntime, OllamaConfig, OllamaRuntime,
};
pub use types::*;
pub use verify::VerificationEngine;

/// Create a model manager with default vault at `~/.promptlab/models` or `./data/models`.
pub fn default_manager() -> ModelResult<LocalModelManager> {
    let vault = std::env::var("PROMPTLAB_MODEL_VAULT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("./data/models"));
    LocalModelManager::new(vault)
}
