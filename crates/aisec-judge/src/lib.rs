//! AISec AI Judge Engine.
//!
//! Rule-based, regex, and offline LLM evaluation with multi-model consensus
//! and confidence scoring. Integrates with `aisec-models` llama.cpp runtime.

pub mod config;
pub mod consensus;
pub mod engine;
pub mod error;
pub mod evaluators;
pub mod factory;
pub mod mock_runtime;
pub mod prompts;
pub mod providers;
pub mod roles;
pub mod runtime_context;
pub mod scoring;
pub mod types;

pub use config::{
    JudgeConnectivityResult, JudgeProviderConfig, LocalProvider, LocalProviderSettings,
    RemoteProvider, RemoteProviderSettings,
};
pub use consensus::ConsensusEngine;
pub use engine::JudgeEngine;
pub use error::{JudgeError, JudgeResult};
pub use evaluators::{LlmEvaluator, RegexEvaluator, RuleBasedEvaluator};
pub use factory::{build_judge_engine, test_connectivity, test_model};
pub use mock_runtime::JsonMockRuntime;
pub use roles::ModelRolePool;
pub use runtime_context::JudgeRuntimeContext;
pub use types::*;

/// Build a judge engine with default config and empty model pool (deterministic-only).
pub fn deterministic_engine() -> JudgeEngine {
    JudgeEngine::with_pool(ModelRolePool::new())
}
