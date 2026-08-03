//! Capability-based Yazg assistant routing.
//!
//! ```text
//! User → LLM IntentRouter → CapabilityToolLoader → AI Runtime (+ capability tools) → optional tool → Response
//! ```
//!
//! The tool-calling LLM never receives the full application tool registry — only tools
//! for the capability chosen by the classifier LLM.

pub mod capability_loader;
pub mod capability_registry;
pub mod router;

pub use capability_loader::{CapabilityToolLoader, LoadedCapabilityTools};
pub use capability_registry::{
    default_capability_registry, AssistantCapability, CapabilityDefinition, CapabilityRegistry,
};
pub use router::{IntentResolution, IntentRouter, RouteInput};
