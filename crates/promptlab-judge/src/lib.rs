//! PromptLab AI Judge Engine.
//!
//! LLM-based evaluation with multi-role consensus and confidence scoring.
//! Integrates with `promptlab-models` / AI Inference Gateway.

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
    JudgeConnectivityResult, JudgeProviderConfig, LocalProviderSettings, RemoteProvider,
    RemoteProviderSettings,
};
pub use consensus::ConsensusEngine;
pub use engine::JudgeEngine;
pub use error::{JudgeError, JudgeResult};
pub use evaluators::{LlmEvaluator, LlmResponseParser};
pub use factory::{
    build_judge_engine, build_judge_engine_with_adapter, build_judge_engine_with_client,
    test_connectivity, test_model,
};
pub use mock_runtime::JsonMockRuntime;
pub use roles::ModelRolePool;
pub use runtime_context::JudgeRuntimeContext;
pub use types::*;
