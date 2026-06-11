//! AISec Attack Framework.
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
pub mod traits;
pub mod transport;
pub mod types;

pub use category::AttackCategory;
pub use collector::{ResultCollector, ResultSink};
pub use error::{AttackError, AttackResult};
pub use executor::AttackExecutor;
pub use lifecycle::{AttackLifecycle, AttackPhase, LifecycleEvent};
pub use orchestrator::{AttackOrchestrator, OrchestratorConfig};
pub use payload::{MutatorConfig, PayloadMutator, PayloadRunner};
pub use registry::AttackRegistry;
#[cfg(feature = "storage")]
pub use scanner::{PromptInjectionScanner, ScanContext, ScanSummary};
pub use traits::Attack;
pub use transport::{HttpTransport, MockTransport, TargetTransport, TransportRequest, TransportResponse};
pub use types::*;

/// Pre-built registry with all built-in attack categories.
pub fn default_registry() -> AttackRegistry {
    AttackRegistry::with_builtins()
}

/// Pre-built executor wired to the default registry and HTTP transport.
pub fn default_executor() -> AttackExecutor<HttpTransport> {
    AttackExecutor::new(default_registry(), HttpTransport::new())
}
