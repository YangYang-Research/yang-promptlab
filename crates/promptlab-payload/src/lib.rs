//! PromptLab Payload Engine — static library, mutations, and generation pipeline.
//!
//! Provides a catalog of adversarial payloads, encoding/obfuscation mutations,
//! and a pipeline to produce attack-ready variants.

pub mod error;
pub mod library;
pub mod mutation;
pub mod pipeline;
pub mod types;

pub use error::{PayloadError, PayloadResult};
pub use library::{parse_category, CatalogSeedEntry, PayloadDatabase};
pub use mutation::{
    base64_encode, bidi_override, caesar_cipher, capitalization_shuffle, crossover_wrap,
    disemvowel, expand_after_wrap, expand_before_wrap, fullwidth_ascii, hex_encode, html_encode,
    inject_prefix_wrap, language_pivot, math_alphanumeric, morse_encode, refusal_suppression_wrap,
    rule_rephrase, shorten_payload, tag_char_smuggle, unicode_obfuscate, url_encode,
    zero_width_variants, llm_crossover_prompt, llm_few_shot_prompt, llm_rephrase_prompt,
    llm_transfer_prompt, sanitize_llm_mutator_output, LLM_MUTATOR_SYSTEM,
    MutatedVariant, MutationConfig, MutationEngine,
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
