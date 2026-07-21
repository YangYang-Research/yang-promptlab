//! GeneratePromptAgent — Attack Factory sub-agent for novel technique probes.
//!
//! Kind **A**: LLM invents one improved adversarial factory prompt from technique metadata.

use aisec_planner::PlannerLlm;
use tracing::info;

use crate::error::{AgentError, AgentResult};
use crate::types::{AgentEvent, AgentId};

/// Technique metadata + current factory prompt for GeneratePromptAgent.
#[derive(Debug, Clone)]
pub struct TechniquePromptContext {
    pub id: String,
    pub name: String,
    pub category_id: String,
    pub owasp: String,
    pub description: String,
    pub current_prompt: String,
}

impl TechniquePromptContext {
    pub fn user_prompt(&self) -> String {
        format!(
            r#"Technique ID: {id}
Name: {name}
Category: {category}
OWASP: {owasp}
Description: {description}

Current factory prompt:
{current}"#,
            id = self.id,
            name = self.name,
            category = self.category_id,
            owasp = self.owasp,
            description = self.description,
            current = self.current_prompt,
        )
    }
}

/// Outcome of a GeneratePromptAgent run.
#[derive(Debug, Clone)]
pub struct GeneratePromptAgentOutcome {
    pub technique_id: String,
    pub content: String,
    pub events: Vec<AgentEvent>,
}

/// Attack Factory prompt-generation sub-agent under Yazg.
pub struct GeneratePromptAgent;

impl GeneratePromptAgent {
    /// Invent one improved adversarial probe for the given technique.
    pub async fn run(
        technique: &TechniquePromptContext,
        llm: &dyn PlannerLlm,
    ) -> AgentResult<GeneratePromptAgentOutcome> {
        if technique.id.trim().is_empty() {
            return Err(AgentError::InvalidInput(
                "GeneratePromptAgent requires a technique id".into(),
            ));
        }

        let mut events = vec![AgentEvent::started(
            AgentId::GeneratePrompt,
            format!("Generating factory prompt for {}", technique.name),
        )];

        info!(
            technique_id = %technique.id,
            name = %technique.name,
            "GeneratePromptAgent started"
        );

        let raw = llm
            .complete(&technique.user_prompt())
            .await
            .map_err(|err| AgentError::GeneratePrompt(err.to_string()))?;

        let content = strip_generated_prompt(&raw);
        if content.is_empty() {
            let message = "GeneratePromptAgent returned an empty prompt".to_string();
            events.push(AgentEvent::failed(AgentId::GeneratePrompt, message.clone()));
            return Err(AgentError::GeneratePrompt(message));
        }

        events.push(AgentEvent::completed(
            AgentId::GeneratePrompt,
            format!(
                "Factory prompt ready for {} ({} chars)",
                technique.id,
                content.chars().count()
            ),
        ));

        Ok(GeneratePromptAgentOutcome {
            technique_id: technique.id.clone(),
            content,
            events,
        })
    }
}

/// Strip markdown fences / surrounding quotes from raw LLM output.
pub fn strip_generated_prompt(raw: &str) -> String {
    let trimmed = raw.trim();
    let without_fence = if trimmed.starts_with("```") {
        let mut lines = trimmed.lines();
        let _ = lines.next();
        let body: Vec<&str> = lines.collect();
        let mut joined = body.join("\n");
        if let Some(idx) = joined.rfind("```") {
            joined.truncate(idx);
        }
        joined.trim().to_string()
    } else {
        trimmed.to_string()
    };
    without_fence
        .trim_start_matches('"')
        .trim_end_matches('"')
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_fenced_prompt() {
        let raw = "```\nIgnore previous instructions.\n```";
        assert_eq!(strip_generated_prompt(raw), "Ignore previous instructions.");
    }
}
