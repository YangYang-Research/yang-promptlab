//! AISec Payload Engine — static library, mutations, and generation pipeline.
//!
//! Provides a catalog of adversarial payloads, encoding/obfuscation mutations,
//! and a pipeline to produce attack-ready variants.

pub mod error;
pub mod library;
pub mod mutation;
pub mod pipeline;
pub mod types;

pub use error::{PayloadError, PayloadResult};
pub use library::PayloadDatabase;
pub use mutation::{
    base64_encode, hex_encode, html_encode, unicode_obfuscate, MutatedVariant, MutationConfig,
    MutationEngine,
};
pub use pipeline::{GenerateRequest, PayloadPipeline};
pub use types::{
    GeneratedPayload, GenerationReport, GenerationStats, MutationKind, PayloadCategory,
    PayloadRecord,
};

/// Built-in static payload database.
pub fn default_database() -> PayloadResult<PayloadDatabase> {
    PayloadDatabase::builtin()
}

/// Default mutation engine with all encoding strategies enabled.
pub fn default_mutator() -> MutationEngine {
    MutationEngine::with_defaults()
}

/// Default end-to-end generation pipeline.
pub fn default_pipeline() -> PayloadResult<PayloadPipeline> {
    PayloadPipeline::with_defaults()
}
