use serde::{Deserialize, Serialize};

/// Payload mutation strategy applied at attack-time HTTP expand.
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
    HexWrap,
    HtmlWrap,
    Rot13Wrap,
    Leetspeak,
    ReversedText,
    TokenSplit,
    MarkdownCodeFence,
    ZeroWidthDense,
    LanguagePivot,
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
            Self::HexWrap,
            Self::HtmlWrap,
            Self::Rot13Wrap,
            Self::Leetspeak,
            Self::ReversedText,
            Self::TokenSplit,
            Self::MarkdownCodeFence,
            Self::ZeroWidthDense,
            Self::LanguagePivot,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Base64Wrap => "base64_wrap",
            Self::UnicodeHomoglyph => "unicode_homoglyph",
            Self::DelimiterInjection => "delimiter_injection",
            Self::RoleSwap => "role_swap",
            Self::ChunkSplit => "chunk_split",
            Self::JsonEscape => "json_escape",
            Self::RepeatAmplify => "repeat_amplify",
            Self::HexWrap => "hex_wrap",
            Self::HtmlWrap => "html_wrap",
            Self::Rot13Wrap => "rot13_wrap",
            Self::Leetspeak => "leetspeak",
            Self::ReversedText => "reversed_text",
            Self::TokenSplit => "token_split",
            Self::MarkdownCodeFence => "markdown_code_fence",
            Self::ZeroWidthDense => "zero_width_dense",
            Self::LanguagePivot => "language_pivot",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "base64_wrap" | "base64" => Some(Self::Base64Wrap),
            "unicode_homoglyph" | "unicode" | "homoglyph" => Some(Self::UnicodeHomoglyph),
            "delimiter_injection" | "delimiter" => Some(Self::DelimiterInjection),
            "role_swap" | "roleswap" => Some(Self::RoleSwap),
            "chunk_split" | "chunk" => Some(Self::ChunkSplit),
            "json_escape" | "json" => Some(Self::JsonEscape),
            "repeat_amplify" | "repeat" => Some(Self::RepeatAmplify),
            "hex_wrap" | "hex" => Some(Self::HexWrap),
            "html_wrap" | "html" => Some(Self::HtmlWrap),
            "rot13_wrap" | "rot13" => Some(Self::Rot13Wrap),
            "leetspeak" | "leet" => Some(Self::Leetspeak),
            "reversed_text" | "reverse" | "flip" => Some(Self::ReversedText),
            "token_split" | "split" => Some(Self::TokenSplit),
            "markdown_code_fence" | "markdown" | "code_fence" => Some(Self::MarkdownCodeFence),
            "zero_width_dense" | "zero_width" | "zwsp" => Some(Self::ZeroWidthDense),
            "language_pivot" | "language" | "translate" | "multilingual" => {
                Some(Self::LanguagePivot)
            }
            _ => None,
        }
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
