use aisec_attack::AttackCategory;

/// Map fingerprint recommendation category strings to attack engine categories.
pub fn normalize_fingerprint_category(raw: &str) -> Option<AttackCategory> {
    match raw {
        "prompt_injection" => Some(AttackCategory::PromptInjection),
        "jailbreak" => Some(AttackCategory::Jailbreak),
        "system_prompt_leakage" | "system_prompt_extraction" => {
            Some(AttackCategory::SystemPromptExtraction)
        }
        "rag_leakage" => Some(AttackCategory::RagLeakage),
        "tool_abuse" => Some(AttackCategory::ToolAbuse),
        "mcp_abuse" => Some(AttackCategory::McpAbuse),
        "memory_poisoning" => Some(AttackCategory::MemoryPoisoning),
        "agent_goal_hijacking" => Some(AttackCategory::AgentGoalHijacking),
        "cross_user_leakage" => Some(AttackCategory::CrossUserLeakage),
        "data_exfiltration" => Some(AttackCategory::SystemPromptExtraction),
        "policy_bypass" => Some(AttackCategory::Jailbreak),
        _ => None,
    }
}

pub fn parse_attack_category(raw: &str) -> Option<AttackCategory> {
    AttackCategory::all()
        .iter()
        .copied()
        .find(|cat| cat.as_str() == raw)
}
