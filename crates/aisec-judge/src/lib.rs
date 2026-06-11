//! AISec AI Judge Engine.
//!
//! Rule-based, regex, and offline LLM evaluation with multi-model consensus
//! and confidence scoring. Integrates with `aisec-models` llama.cpp runtime.

pub mod consensus;
pub mod engine;
pub mod error;
pub mod evaluators;
pub mod mock_runtime;
pub mod prompts;
pub mod roles;
pub mod scoring;
pub mod types;

pub use consensus::ConsensusEngine;
pub use engine::JudgeEngine;
pub use error::{JudgeError, JudgeResult};
pub use evaluators::{LlmEvaluator, RegexEvaluator, RuleBasedEvaluator};
pub use mock_runtime::JsonMockRuntime;
pub use roles::ModelRolePool;
pub use types::*;

/// Build a judge engine with default config and empty model pool (deterministic-only).
pub fn deterministic_engine() -> JudgeEngine {
    JudgeEngine::with_pool(ModelRolePool::new())
}
