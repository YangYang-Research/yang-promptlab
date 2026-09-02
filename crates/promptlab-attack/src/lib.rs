//! PromptLab Attack Framework.
//!
//! Trait-based attack plugins with payload execution, mutation, orchestration,
//! and structured result collection.

pub mod attacks;
pub mod category;
pub mod collector;
pub mod error;
pub mod executor;
pub mod lifecycle;
pub mod orchestrator;
pub mod payload;
pub mod registry;
#[cfg(feature = "storage")]
pub mod scanner;
pub mod target_auth;
pub mod traits;
pub mod transport;
pub mod types;

pub use category::AttackCategory;
pub use collector::{ResultCollector, ResultSink};
pub use error::{AttackError, AttackResult};
pub use executor::{AttackExecutor, AttemptStreamItem};
pub use lifecycle::{AttackLifecycle, AttackPhase, LifecycleEvent};
pub use orchestrator::{AttackOrchestrator, OrchestratorConfig};
pub use payload::{LlmComplete, MutatorConfig, MutatorKind, PayloadMutator, PayloadRunner};
pub use registry::AttackRegistry;
#[cfg(feature = "storage")]
pub use scanner::{PromptInjectionScanner, ScanContext, ScanSummary};
pub use attacks::{
    merge_canary_evaluation, payload_canary, preserve_canary_in_mutated, stamp_payload_canary,
};
pub use traits::Attack;
pub use target_auth::{apply_descriptor_auth, apply_descriptor_auth_value};
pub use transport::{HarnessTransport, MockTransport, TargetTransport, TransportRequest, TransportResponse};
pub use types::{DEFAULT_ATTACK_CONCURRENCY, *};

/// Pre-built registry with all built-in attack categories.
pub fn default_registry() -> AttackRegistry {
    AttackRegistry::with_builtins()
}

/// Pre-built executor wired to the default registry and harness transport.
pub fn default_executor_for(
    endpoint_url: impl AsRef<str>,
) -> AttackResult<AttackExecutor<HarnessTransport>> {
    let transport = HarnessTransport::for_attack_target(&AttackTarget::llm_api(endpoint_url.as_ref()))?;
    Ok(AttackExecutor::new(default_registry(), transport))
}
