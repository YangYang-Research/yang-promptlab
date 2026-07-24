use serde::{Deserialize, Serialize};

/// Payload category aligned with PromptLab attack taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayloadCategory {
    PromptInjection,
    SystemPromptExtraction,
    Jailbreak,
    RagLeakage,
    MemoryPoisoning,
    CrossUserLeakage,
    AgentGoalHijacking,
    ToolAbuse,
    McpAbuse,
    Encoding, // encoding-focused probes
}

impl PayloadCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PromptInjection => "prompt_injection",
            Self::SystemPromptExtraction => "system_prompt_extraction",
            Self::Jailbreak => "jailbreak",
            Self::RagLeakage => "rag_leakage",
            Self::MemoryPoisoning => "memory_poisoning",
            Self::CrossUserLeakage => "cross_user_leakage",
            Self::AgentGoalHijacking => "agent_goal_hijacking",
            Self::ToolAbuse => "tool_abuse",
            Self::McpAbuse => "mcp_abuse",
            Self::Encoding => "encoding",
        }
    }

    pub fn all() -> &'static [PayloadCategory] {
        use PayloadCategory::*;
        &[
            PromptInjection,
            SystemPromptExtraction,
            Jailbreak,
            RagLeakage,
            MemoryPoisoning,
            CrossUserLeakage,
            AgentGoalHijacking,
            ToolAbuse,
            McpAbuse,
            Encoding,
        ]
    }
}

impl std::fmt::Display for PayloadCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Static payload record from the library catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayloadRecord {
    pub id: String,
    pub name: String,
    pub category: PayloadCategory,
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Mutation strategy applied to payload content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationKind {
    UnicodeObfuscation,
    Base64Encode,
    HexEncode,
    HtmlEncode,
    Base64Wrap,
    HexWrap,
    HtmlWrap,
}

impl MutationKind {
    pub fn encoding_kinds() -> &'static [MutationKind] {
        &[
            Self::UnicodeObfuscation,
            Self::Base64Encode,
            Self::HexEncode,
            Self::HtmlEncode,
        ]
    }

    pub fn all() -> &'static [MutationKind] {
        &[
            Self::UnicodeObfuscation,
            Self::Base64Encode,
            Self::HexEncode,
            Self::HtmlEncode,
            Self::Base64Wrap,
            Self::HexWrap,
            Self::HtmlWrap,
        ]
    }
}

/// A generated payload variant with mutation lineage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedPayload {
    pub generation_id: String,
    pub source_id: String,
    pub source_name: String,
    pub category: PayloadCategory,
    pub content: String,
    pub mutations: Vec<MutationKind>,
}

/// Statistics from a generation run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GenerationStats {
    pub source_count: usize,
    pub variant_count: usize,
    pub mutations_applied: usize,
}

/// Output of the payload generation pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationReport {
    pub variants: Vec<GeneratedPayload>,
    pub stats: GenerationStats,
}
