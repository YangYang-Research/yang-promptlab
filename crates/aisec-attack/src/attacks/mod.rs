mod agent_goal_hijacking;
mod common;
mod cross_user_leakage;
mod jailbreak;
mod mcp_abuse;
mod memory_poisoning;
mod prompt_injection;
mod rag_leakage;
mod system_prompt_extraction;
mod tool_abuse;

use std::sync::Arc;

use crate::traits::Attack;

pub use agent_goal_hijacking::AgentGoalHijackingAttack;
pub use cross_user_leakage::CrossUserLeakageAttack;
pub use jailbreak::JailbreakAttack;
pub use mcp_abuse::McpAbuseAttack;
pub use memory_poisoning::MemoryPoisoningAttack;
pub use prompt_injection::PromptInjectionAttack;
pub use rag_leakage::RagLeakageAttack;
pub use system_prompt_extraction::SystemPromptExtractionAttack;
pub use tool_abuse::ToolAbuseAttack;

/// All built-in attack implementations.
pub fn builtin_attacks() -> Vec<Arc<dyn Attack>> {
    vec![
        Arc::new(PromptInjectionAttack),
        Arc::new(SystemPromptExtractionAttack),
        Arc::new(JailbreakAttack),
        Arc::new(RagLeakageAttack),
        Arc::new(MemoryPoisoningAttack),
        Arc::new(CrossUserLeakageAttack),
        Arc::new(AgentGoalHijackingAttack),
        Arc::new(ToolAbuseAttack),
        Arc::new(McpAbuseAttack),
    ]
}
