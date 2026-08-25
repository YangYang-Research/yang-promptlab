//! PromptLab model registry and vault helpers.
//!
//! Remote/third-party provider registry, optional HuggingFace download plumbing,
//! and Ollama-over-HTTP smoke tests. Embedded llama.cpp / in-process GGUF is removed.
//! The built-in GGUF `models.json` catalog has been removed — Remote-only.

pub mod download;
pub mod error;
pub mod hardware;
pub mod import_pack;
pub mod manager;
pub mod registry;
pub mod runtime;
pub mod types;
pub mod verify;

pub use download::{DownloadControl, DownloadCoordinator, DownloadManager, DownloadOptions, HuggingFaceClient, huggingface_url, PipelinePhase};
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
