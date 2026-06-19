//! AISec Local Model Manager.
//!
//! GGUF model registry, HuggingFace downloads with resume, SHA256 verification,
//! hardware/GPU detection, and llama.cpp runtime integration.

pub mod builtin_catalog;
pub mod catalog;
pub mod download;
pub mod error;
pub mod hardware;
pub mod import_pack;
pub mod manager;
pub mod registry;
pub mod registry_validate;
pub mod runtime;
pub mod types;
pub mod verify;

pub use builtin_catalog::{BuiltinCatalog, BuiltinCatalogMeta, BuiltinRegistryEntry, entry_to_catalog};
pub use catalog::find_catalog_entry;
pub use download::{DownloadControl, DownloadCoordinator, DownloadManager, DownloadOptions, HuggingFaceClient, huggingface_url};
pub use error::{ModelError, ModelResult};
pub use hardware::detect_hardware;
pub use manager::LocalModelManager;
pub use registry::ModelRegistry;
pub use runtime::{
    InferenceRuntime, LocalInferenceEngine, LlamaCppConfig, LlamaCppRuntime,
    MockInferenceRuntime, OllamaConfig, OllamaRuntime,
};
#[cfg(feature = "llama")]
pub use runtime::{LlamaInProcessRuntime, LlamaModelConfig};
pub use types::*;
pub use registry_validate::{validate_registry, RegistryValidationIssue, RegistryValidationReport};
pub use verify::VerificationEngine;

/// Create a model manager with default vault at `~/.aisec/models` or `./data/models`.
pub fn default_manager() -> ModelResult<LocalModelManager> {
    let vault = std::env::var("AISEC_MODEL_VAULT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("./data/models"));
    LocalModelManager::new(vault)
}
