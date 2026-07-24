//! Dynamic prompt payload generation from attack plans.

pub mod advanced;
pub mod convert;
pub mod engine;
pub mod error;
pub mod local_llm;
pub mod static_pack;
pub mod template_mutation;
pub mod types;

pub use advanced::{apply_advanced_options, feedback_from_judged};
pub use engine::{generate_from_plan, generate_prompt_payloads, generate_prompt_payloads_with_llm};
pub use error::{GeneratorError, GeneratorResult};
pub use local_llm::{generate_with_local_llm, GeneratorLlm};
pub use types::{
    GeneratePayloadsInput, GeneratorAdvancedOptions, GeneratorMode, GeneratorStats,
    GeneratorTargetContext, PromptPayloads,
};
