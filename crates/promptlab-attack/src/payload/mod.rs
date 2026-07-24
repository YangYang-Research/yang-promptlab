use serde::{Deserialize, Serialize};

/// Payload mutation strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutatorKind {
    Base64Wrap,
    UnicodeHomoglyph,
    DelimiterInjection,
    RoleSwap,
    ChunkSplit,
    JsonEscape,
    RepeatAmplify,
}

impl MutatorKind {
    pub fn all() -> &'static [MutatorKind] {
        &[
            Self::Base64Wrap,
            Self::UnicodeHomoglyph,
            Self::DelimiterInjection,
            Self::RoleSwap,
            Self::ChunkSplit,
            Self::JsonEscape,
            Self::RepeatAmplify,
        ]
    }
}

/// Configuration for payload mutation pipeline.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MutatorConfig {
    pub enabled: Vec<MutatorKind>,
    pub max_per_payload: usize,
}

mod mutator;
mod runner;

pub use mutator::PayloadMutator;
pub use runner::PayloadRunner;
