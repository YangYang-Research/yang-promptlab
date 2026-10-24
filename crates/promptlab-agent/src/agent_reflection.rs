//! One reflection pass after an agent produces text.
//!
//! Text agents ([`ReflectingLlm`]) rewrite while ReflectionAgent rejects the answer, up to the attempt budget (minimum 3).
//! Scan execution already reflects inside AgenticAttackExecutionAgent.
//! JailbreakAgent plan rewrites a rejected seed on its own loop.
//! JailbreakAgent follow-up uses this reflector on the model output.
//! Judge votes are reviewed and left unchanged.

use promptlab_planner::{LlmCompletion, PlannerLlm, ToolSpec};
use serde_json::Value;
use tokio::task_local;

use crate::types::{AgentEvent, AgentId};

task_local! {
    static IN_OUTPUT_REFLECTION: bool;
}

/// Wraps a specialist LLM so each text completion is reflected once.
pub struct ReflectingLlm<'a> {
    agent: AgentId,
    inner: &'a dyn PlannerLlm,
}

impl<'a> ReflectingLlm<'a> {
    pub fn new(agent: AgentId, inner: &'a dyn PlannerLlm) -> Self {
        Self { agent, inner }
    }
}

#[async_trait::async_trait]
impl PlannerLlm for ReflectingLlm<'_> {
    async fn complete(&self, prompt: &str) -> promptlab_planner::PlannerResult<String> {
        let first = self.inner.complete(prompt).await?;
        Ok(revise_text(self.agent, prompt, first, self.inner).await)
    }

    async fn complete_with_system(
        &self,
        system: Option<&str>,
        prompt: &str,
    ) -> promptlab_planner::PlannerResult<String> {
        let first = self.inner.complete_with_system(system, prompt).await?;
        Ok(revise_text(self.agent, prompt, first, self.inner).await)
    }

    async fn complete_with_tools(
        &self,
        prompt: &str,
        tools: &[ToolSpec],
    ) -> promptlab_planner::PlannerResult<LlmCompletion> {
        self.inner.complete_with_tools(prompt, tools).await
    }

    async fn complete_with_tools_and_system(
        &self,
        system: Option<&str>,
        prompt: &str,
        tools: &[ToolSpec],
    ) -> promptlab_planner::PlannerResult<LlmCompletion> {
        self.inner
            .complete_with_tools_and_system(system, prompt, tools)
            .await
    }

    async fn complete_with_tools_messages(
        &self,
        messages: &[Value],
        tools: &[ToolSpec],
    ) -> promptlab_planner::PlannerResult<LlmCompletion> {
        self.inner
            .complete_with_tools_messages(messages, tools)
            .await
    }
}

/// Reflect, and rewrite while ReflectionAgent rejects the answer.
/// Stops after [`crate::agent_attempts::agent_attempts`] tries (minimum 3).
pub async fn revise_text(
    agent: AgentId,
    task: &str,
    output: String,
    llm: &dyn PlannerLlm,
) -> String {
    let limit = crate::agent_attempts::agent_attempts(agent);
    let mut output = output;
    for attempt in 1..=limit {
        let Some(verdict) = reflect(agent, task, &output, llm).await else {
            return output;
        };
        if verdict.accept || attempt == limit {
            return output;
        }
        AgentEvent::react(
            agent,
            format!(
                "Reflection rejected the answer. Attempt {}/{limit}. {}",
                attempt + 1,
                verdict.reason
            ),
        );
        let prompt = revision_prompt(task, &output, &verdict.reason);
        match llm.complete(&prompt).await {
            Ok(text) if !text.trim().is_empty() => output = text,
            _ => return output,
        }
    }
    output
}

/// Log a reflection. Does not change the output (judge votes, sequential stop).
pub async fn reflect_output(agent: AgentId, task: &str, output: &str, llm: &dyn PlannerLlm) {
    let _ = reflect(agent, task, output, llm).await;
}

struct Verdict {
    accept: bool,
    reason: String,
}

fn in_reflection() -> bool {
    IN_OUTPUT_REFLECTION.try_with(|flag| *flag).unwrap_or(false)
}

async fn reflect(
    agent: AgentId,
    task: &str,
    output: &str,
    llm: &dyn PlannerLlm,
) -> Option<Verdict> {
    if agent == AgentId::Reflection || output.trim().is_empty() || in_reflection() {
        return None;
    }
    let system = reflection_system_prompt();
    let user = format!(
        "Agent: {}\nTask:\n{}\n\nAnswer:\n{}",
        agent.display_name(),
        clip(task, 4000),
        clip(output, 4000)
    );
    AgentEvent::started(
        AgentId::Reflection,
        format!("Reflecting on {} output", agent.display_name()),
    );
    let raw = match IN_OUTPUT_REFLECTION
        .scope(true, llm.complete_with_system(Some(system), &user))
        .await
    {
        Ok(text) => text,
        Err(err) => {
            AgentEvent::failed(
                AgentId::Reflection,
                format!("reflection failed for {}: {err}", agent.display_name()),
            );
            return None;
        }
    };
    let verdict = parse_verdict(&raw);
    AgentEvent::completed(
        AgentId::Reflection,
        format!("accept={} — {}", verdict.accept, verdict.reason),
    );
    AgentEvent::react(
        agent,
        format!("Reflection: accept={} — {}", verdict.accept, verdict.reason),
    );
    Some(verdict)
}

fn reflection_system_prompt() -> &'static str {
    promptlab_registry::agent_registry::load(AgentId::Reflection.as_str())
        .and_then(|agent| agent.prompt("agent_output_reflection_system_prompt"))
        .expect("agent_output_reflection_system_prompt")
        .system
}

fn revision_prompt(task: &str, previous: &str, reason: &str) -> String {
    format!(
        "{task}\n\n\
         Your previous answer was rejected.\n\
         Reason: {reason}\n\
         Rejected answer:\n\
         {}\n\n\
         Reflect, then answer the original task again in the same format. \
         Return only that answer.",
        clip(previous, 8000)
    )
}

fn parse_verdict(text: &str) -> Verdict {
    let mut accept = None;
    let mut reason = String::new();
    for line in text.lines() {
        let line = line.trim().trim_start_matches(['-', '*', '#']).trim();
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        if key == "accept" && accept.is_none() {
            accept = Some(!matches!(
                value.to_ascii_lowercase().as_str(),
                "no" | "false" | "n" | "reject"
            ));
        } else if key == "reason" && reason.is_empty() {
            reason = value.to_string();
        }
    }
    match accept {
        Some(accept) => Verdict {
            accept,
            reason: if reason.is_empty() {
                if accept {
                    "accepted".into()
                } else {
                    "rejected".into()
                }
            } else {
                reason
            },
        },
        None => Verdict {
            accept: true,
            reason: "reflection did not return ACCEPT; keeping the answer".into(),
        },
    }
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let shortened: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{shortened}…")
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use super::*;

    #[test]
    fn parses_accept_and_reject() {
        let yes = parse_verdict("ACCEPT: yes\nREASON: task is done");
        assert!(yes.accept);
        assert_eq!(yes.reason, "task is done");
        let no = parse_verdict("ACCEPT: no\nREASON: missed the goal");
        assert!(!no.accept);
        assert_eq!(no.reason, "missed the goal");
    }

    #[test]
    fn missing_accept_keeps_the_answer() {
        let verdict = parse_verdict("here is a prompt");
        assert!(verdict.accept);
    }

    struct Scripted {
        replies: Mutex<VecDeque<String>>,
        calls: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl PlannerLlm for Scripted {
        async fn complete(&self, prompt: &str) -> promptlab_planner::PlannerResult<String> {
            self.calls.lock().unwrap().push(prompt.to_string());
            Ok(self.replies.lock().unwrap().pop_front().unwrap())
        }

        async fn complete_with_system(
            &self,
            system: Option<&str>,
            prompt: &str,
        ) -> promptlab_planner::PlannerResult<String> {
            self.calls
                .lock()
                .unwrap()
                .push(system.unwrap_or("").to_string());
            self.complete(prompt).await
        }
    }

    #[tokio::test]
    async fn reject_retries_until_attempt_limit() {
        let llm = Scripted {
            replies: Mutex::new(VecDeque::from([
                "ACCEPT: no\nREASON: missed the goal".into(),
                "still bad".into(),
                "ACCEPT: no\nREASON: still missed".into(),
                "good answer".into(),
                "ACCEPT: yes\nREASON: done".into(),
            ])),
            calls: Mutex::new(Vec::new()),
        };
        let text = revise_text(AgentId::GeneratePrompt, "write a probe", "bad".into(), &llm).await;
        assert_eq!(text, "good answer");
    }

    #[tokio::test]
    async fn accept_keeps_the_answer() {
        let llm = Scripted {
            replies: Mutex::new(VecDeque::from(["ACCEPT: yes\nREASON: done".into()])),
            calls: Mutex::new(Vec::new()),
        };
        let text = revise_text(AgentId::Summary, "summarize", "overview".into(), &llm).await;
        assert_eq!(text, "overview");
        assert_eq!(llm.calls.lock().unwrap().len(), 2);
    }
}
