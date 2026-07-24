use serde::{Deserialize, Serialize};

/// AI security attack category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttackCategory {
    PromptInjection,
    SystemPromptExtraction,
    Jailbreak,
    RagLeakage,
    MemoryPoisoning,
    CrossUserLeakage,
    AgentGoalHijacking,
    ToolAbuse,
    McpAbuse,
}

impl AttackCategory {
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
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::PromptInjection => "Prompt Injection",
            Self::SystemPromptExtraction => "System Prompt Extraction",
            Self::Jailbreak => "Jailbreak",
            Self::RagLeakage => "RAG Leakage",
            Self::MemoryPoisoning => "Memory Poisoning",
            Self::CrossUserLeakage => "Cross User Leakage",
            Self::AgentGoalHijacking => "Agent Goal Hijacking",
            Self::ToolAbuse => "Tool Abuse",
            Self::McpAbuse => "MCP Abuse",
        }
    }

    pub fn all() -> &'static [AttackCategory] {
        use AttackCategory::*;
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
        ]
    }
}

impl std::fmt::Display for AttackCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_categories_have_stable_ids() {
        assert_eq!(AttackCategory::all().len(), 9);
        for cat in AttackCategory::all() {
            assert!(!cat.as_str().is_empty());
        }
    }
}
