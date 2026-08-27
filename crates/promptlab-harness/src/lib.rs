//! App-wide AI I/O layer for PromptLab.
//!
//! Every completion — attack delivery, wizard verify, discovery, Assistant, Judge,
//! planner, generator, and reports — goes through [`HarnessFactory::execute`].
//! Providers register by id; callers set [`HarnessPurpose`].
//!
//! Attack path still ends at [`NormalizedResponse`] for the Judge Engine.

pub mod adapter;
pub mod cancel;
pub mod error;
pub mod factory;
pub mod models;
pub mod pipeline;
pub mod providers;
pub mod redact;
pub mod registry;
pub mod sigv4;
pub mod sse;
pub mod traits;

pub use adapter::HarnessAttackTransport;
pub use cancel::CancelFlag;
pub use error::{HarnessError, HarnessResult};
pub use factory::HarnessFactory;
pub use models::{
    provider_error_detail, AttackRequest, AuthMaterial, ChatMessage, ChatTool, HarnessKind,
    HarnessPurpose, HarnessRequest, HttpMethod, NormalizedResponse, NormalizedToolCall, StreamChunk,
    TargetDescriptor, TargetSurface,
};
pub use pipeline::{HarnessInterceptor, InterceptAction};
pub use registry::HarnessRegistry;
pub use traits::{Harness, ResponseNormalizer};
pub use providers::{
    AnthropicHarness, BedrockHarness, DifyHarness, GeminiHarness, HttpHarness, LlamaHarness,
    McpHarness, OpenAiHarness, WebSocketHarness,
};
#[cfg(feature = "playwright")]
pub use providers::PlaywrightHarness;

