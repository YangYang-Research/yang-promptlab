//! Context-engineered prompts for Yazg.
//!
//! Text lives in `promptlab-registry`. This module re-exports the ids Yazg binds.

pub use promptlab_registry::prompt_registry::{
    YAZG_CONVERSATION_PREAMBLE, YAZG_KNOWLEDGE_PREAMBLE, YAZG_PREAMBLE,
};
