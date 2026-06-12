//! AI endpoint fingerprinting engine for AISec.
//!
//! Identifies inference providers (OpenAI, Anthropic, Gemini, Bedrock, Azure OpenAI,
//! Ollama, LiteLLM, vLLM) from URL, header, and response body signals.

pub mod engine;
pub mod evaluator;
pub mod openapi;
pub mod rules;
pub mod scoring;
pub mod types;

pub use engine::FingerprintEngine;
pub use openapi::inputs_from_openapi;
pub use rules::rule_catalog;
pub use types::{
    AiProvider, ApiStyle, DEFAULT_CONFIDENCE_THRESHOLD, FingerprintInput, FingerprintReport,
    MatchedSignal, ProviderFingerprint,
};
