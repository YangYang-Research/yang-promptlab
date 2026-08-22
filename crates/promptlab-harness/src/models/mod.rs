mod attack_request;
mod chat;
mod normalized_response;
mod target_descriptor;

pub use attack_request::{AttackRequest, AuthMaterial, HarnessPurpose, HttpMethod};
/// Canonical name for the target I/O request. `AttackRequest` is the same type.
pub type HarnessRequest = AttackRequest;
pub use chat::{ChatMessage, ChatTool, StreamChunk};
pub use normalized_response::{NormalizedResponse, NormalizedToolCall};
pub use target_descriptor::{HarnessKind, TargetDescriptor, TargetSurface};
