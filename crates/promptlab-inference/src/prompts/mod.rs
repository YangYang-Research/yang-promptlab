//! Centralized prompt templates — no inline prompts in feature modules.

mod builder;
mod composer;
mod context;
mod registry;
mod template;

pub use builder::PromptBuilder;
pub use composer::PromptComposer;
pub use context::PromptContext;
pub use registry::PromptRegistry;
pub use template::{PromptId, PromptTemplate};
