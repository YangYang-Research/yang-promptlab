//! Central catalogs for PromptLab agents, prompts, and tools.
//!
//! The app loads an agent from [`agent_registry`]. The agent loads its system
//! prompt from [`prompt_registry`] and its tools from [`tool_registry`] by id.

pub mod agent_registry;
pub mod error;
pub mod prompt_registry;
pub mod prompt_template;
pub mod tool_registry;

pub use error::RegistryError;
pub use prompt_registry::PromptRegistry;
pub use prompt_template::{PromptId, PromptTemplate};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_agent_loads_its_prompts_and_tools() {
        for record in agent_registry::all() {
            let agent = agent_registry::load(record.id).expect(record.id);
            assert_eq!(agent.id(), record.id);
            assert_eq!(agent.display_name(), record.display_name);
            assert!(!agent.description().is_empty());
            for prompt_id in agent.prompt_ids() {
                let prompt = agent.prompt(prompt_id).expect(prompt_id);
                assert_eq!(prompt.id, *prompt_id);
                assert!(!prompt.system.is_empty());
            }
            for tool_id in agent.tool_ids() {
                let tool = agent.tool(tool_id).expect(tool_id);
                assert_eq!(tool.id, *tool_id);
                assert!(tool.description.len() > 80, "{tool_id}");
            }
        }
    }

    #[test]
    fn unknown_ids_fail() {
        assert!(agent_registry::load("not-an-agent").is_err());
        assert!(prompt_registry::load("not-a-prompt").is_err());
        assert!(tool_registry::load("not-a-tool").is_err());
    }
}
