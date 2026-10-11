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
    RefusalSuppression,
    InjectPrefix,
    UrlWrap,
    CaesarWrap,
    MorseWrap,
    FullwidthAscii,
    BidiOverride,
    TagCharSmuggle,
    ZeroWidthVariants,
    MathAlphanumeric,
    Disemvowel,
    ExpandBefore,
    ExpandAfter,
    CapitalizationShuffle,
    Rephrase,
    Shorten,
    Crossover,
    LlmRephrase,
    LlmCrossover,
    LlmFewShot,
    LlmTransfer,
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
            Self::RefusalSuppression,
            Self::InjectPrefix,
            Self::UrlWrap,
            Self::CaesarWrap,
            Self::MorseWrap,
            Self::FullwidthAscii,
            Self::BidiOverride,
            Self::TagCharSmuggle,
            Self::ZeroWidthVariants,
            Self::MathAlphanumeric,
            Self::Disemvowel,
            Self::ExpandBefore,
            Self::ExpandAfter,
            Self::CapitalizationShuffle,
            Self::Rephrase,
            Self::Shorten,
            Self::Crossover,
            Self::LlmRephrase,
            Self::LlmCrossover,
            Self::LlmFewShot,
            Self::LlmTransfer,
        ]
    }

    pub fn is_llm(self) -> bool {
        matches!(
            self,
            Self::LlmRephrase | Self::LlmCrossover | Self::LlmFewShot | Self::LlmTransfer
        )
    }

    pub fn llm_kinds() -> &'static [MutatorKind] {
        &[
            Self::LlmRephrase,
            Self::LlmCrossover,
            Self::LlmFewShot,
            Self::LlmTransfer,
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
            Self::RefusalSuppression => "refusal_suppression",
            Self::InjectPrefix => "inject_prefix",
            Self::UrlWrap => "url_wrap",
            Self::CaesarWrap => "caesar_wrap",
            Self::MorseWrap => "morse_wrap",
            Self::FullwidthAscii => "fullwidth_ascii",
            Self::BidiOverride => "bidi_override",
            Self::TagCharSmuggle => "tag_char_smuggle",
            Self::ZeroWidthVariants => "zero_width_variants",
            Self::MathAlphanumeric => "math_alphanumeric",
            Self::Disemvowel => "disemvowel",
            Self::ExpandBefore => "expand_before",
            Self::ExpandAfter => "expand_after",
            Self::CapitalizationShuffle => "capitalization_shuffle",
            Self::Rephrase => "rephrase",
            Self::Shorten => "shorten",
            Self::Crossover => "crossover",
            Self::LlmRephrase => "llm_rephrase",
            Self::LlmCrossover => "llm_crossover",
            Self::LlmFewShot => "llm_few_shot",
            Self::LlmTransfer => "llm_transfer",
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
            "refusal_suppression" | "refusal" => Some(Self::RefusalSuppression),
            "inject_prefix" | "prefix_inject" => Some(Self::InjectPrefix),
            "url_wrap" | "url_encode" => Some(Self::UrlWrap),
            "caesar_wrap" | "caesar" | "caesar_cipher" => Some(Self::CaesarWrap),
            "morse_wrap" | "morse" => Some(Self::MorseWrap),
            "fullwidth_ascii" | "fullwidth" => Some(Self::FullwidthAscii),
            "bidi_override" | "bidi" => Some(Self::BidiOverride),
            "tag_char_smuggle" | "tag_char" | "unicode_tag" => Some(Self::TagCharSmuggle),
            "zero_width_variants" | "zw_variants" => Some(Self::ZeroWidthVariants),
            "math_alphanumeric" | "math_alpha" => Some(Self::MathAlphanumeric),
            "disemvowel" | "vowel_strip" => Some(Self::Disemvowel),
            "expand_before" => Some(Self::ExpandBefore),
            "expand_after" => Some(Self::ExpandAfter),
            "capitalization_shuffle" | "caps_shuffle" => Some(Self::CapitalizationShuffle),
            "rephrase" => Some(Self::Rephrase),
            "shorten" => Some(Self::Shorten),
            "crossover" => Some(Self::Crossover),
            "llm_rephrase" => Some(Self::LlmRephrase),
            "llm_crossover" => Some(Self::LlmCrossover),
            "llm_few_shot" | "llm_fewshot" | "few_shot" => Some(Self::LlmFewShot),
            "llm_transfer" | "llm_translate" => Some(Self::LlmTransfer),
            _ => None,
        }
    }

    /// Encoding-style mutators commonly chained after structural transforms.
    pub fn chain_secondary_kinds() -> &'static [MutatorKind] {
        &[
            Self::Base64Wrap,
            Self::UrlWrap,
            Self::UnicodeHomoglyph,
            Self::ZeroWidthVariants,
            Self::TagCharSmuggle,
            Self::HexWrap,
            Self::HtmlWrap,
        ]
    }
}

/// Configuration for payload mutation pipeline.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MutatorConfig {
    pub enabled: Vec<MutatorKind>,
    pub max_per_payload: usize,
    /// When > 0, also emit chained variants (structural + encoding layers).
    #[serde(default)]
    pub chain_depth: u8,
}

mod llm;
mod mutator;
mod runner;

pub use llm::{apply_llm_mutator, LlmComplete};
pub use mutator::PayloadMutator;
pub use runner::PayloadRunner;
