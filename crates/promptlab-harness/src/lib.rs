//! Execution harness layer for PromptLab.
//!
//! Routes attack payloads to heterogeneous targets (HTTP APIs, OpenAI-compatible
//! endpoints, browser chat UIs) and returns a [`NormalizedResponse`] that the
//! Judge Engine consumes without knowing which harness executed the attack.

pub mod adapter;
pub mod error;
pub mod factory;
pub mod models;
pub mod providers;
pub mod registry;
pub mod traits;

pub use adapter::HarnessAttackTransport;
pub use error::{HarnessError, HarnessResult};
pub use factory::HarnessFactory;
pub use models::{
    AttackRequest, AuthMaterial, HarnessKind, HttpMethod, NormalizedResponse, TargetDescriptor,
    TargetSurface,
};
pub use registry::HarnessRegistry;
pub use traits::{Harness, ResponseNormalizer};
pub use providers::{HttpHarness, OpenAiHarness};
#[cfg(feature = "playwright")]
pub use providers::PlaywrightHarness;
