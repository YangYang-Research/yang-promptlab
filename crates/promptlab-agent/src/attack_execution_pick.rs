//! Orchestrator action-pick helpers for scan orchestrators.
//!
//! Host tools (`AttackExecutionTools`) are borrowed (`&dyn`) from the Tauri scan
//! host and are therefore non-`'static`, so they cannot be registered directly on
//! an AgentBuilder agent. The pick tools choose an action; the host loop executes against
//! `&dyn AttackExecutionTools` (same observation contract as a full in-agent tool).
//!
//! Domain specialists are tools on Yazg.

use std::future::IntoFuture;
use std::sync::Arc;

use promptlab_planner::PlannerLlm;
use rig::agent::AgentBuilder;
use rig::completion::Prompt;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

use crate::yazg_model::YazgModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackPickAction {
    Generate,
    Attack,
    Recover,
    Reflect,
    Adapt,
    Finish,
}

struct ActionPick {
    action: AttackPickAction,
    reply: oneshot::Sender<String>,
}

#[derive(Debug, Error)]
#[error("{0}")]
struct PickError(String);

#[derive(Deserialize, Serialize, Default)]
struct EmptyArgs {}

macro_rules! pick_tool {
    ($name:ident, $const:expr, $action:expr, $desc:expr) => {
        struct $name {
            tx: mpsc::Sender<ActionPick>,
        }

        impl Tool for $name {
            const NAME: &'static str = $const;
            type Error = PickError;
            type Args = EmptyArgs;
            type Output = String;

            fn description(&self) -> String {
                $desc.into()
            }

            fn parameters(&self) -> serde_json::Value {
                json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                })
            }

            async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
                let (reply_tx, reply_rx) = oneshot::channel();
                self.tx
                    .send(ActionPick {
                        action: $action,
                        reply: reply_tx,
                    })
                    .await
                    .map_err(|_| PickError("action channel closed".into()))?;
                reply_rx
                    .await
                    .map_err(|_| PickError("action reply dropped".into()))
            }
        }
    };
}

pick_tool!(
    GeneratePickTool,
    "generate",
    AttackPickAction::Generate,
    "Generate / regenerate payloads for the next attack attempt."
);
pick_tool!(
    AttackPickTool,
    "attack",
    AttackPickAction::Attack,
    "Run HTTP attack+judge for the current attempt."
);
pick_tool!(
    RecoverPickTool,
    "recover",
    AttackPickAction::Recover,
    "Adjust endpoint pacing after an unhealthy attack observation."
);
pick_tool!(
    ReflectPickTool,
    "reflect",
    AttackPickAction::Reflect,
    "Decide whether to retry after a healthy attack observation."
);
pick_tool!(
    AdaptPickTool,
    "adapt",
    AttackPickAction::Adapt,
    "Adapt payload strategy after reflect says retry."
);

/// One LLM turn only: either a single tool pick or plain-text Finish.
/// A second tool turn deadlocks — the host stops `rx.recv()` after the first pick
/// while the tool awaits a oneshot reply that never comes.
const PICK_MAX_TURNS: usize = 1;

async fn await_first_pick(
    mut rx: mpsc::Receiver<ActionPick>,
    prompt_fut: impl std::future::Future<Output = Result<String, rig::completion::PromptError>>,
) -> Result<(String, AttackPickAction), String> {
    tokio::pin!(prompt_fut);

    tokio::select! {
        biased;
        Some(pick) = rx.recv() => {
            let action = pick.action;
            let _ = pick.reply.send("acknowledged — host will execute".into());
            // Drop the agent future immediately. Awaiting it after a pick can deadlock
            // if the model issues another tool call (tool waits on oneshot; we no longer recv).
            drop(prompt_fut);
            // Close the pick channel so any late tool call fails fast instead of hanging.
            drop(rx);
            Ok(("(orchestrator)".into(), action))
        }
        result = &mut prompt_fut => {
            match result {
                Ok(text) => {
                    let mut thought = text.trim().to_string();
                    if thought.is_empty() {
                        thought = "finish".into();
                    }
                    Ok((thought, AttackPickAction::Finish))
                }
                Err(err) => Err(err.to_string()),
            }
        }
    }
}

/// Ask the orchestrator LLM to pick the next agentic action (one tool call), or Finish on text.
pub async fn pick_agentic_action(
    llm: Arc<dyn PlannerLlm>,
    transcript: &str,
    step: usize,
    max_steps: usize,
) -> Result<(String, AttackPickAction), String> {
    let (tx, rx) = mpsc::channel::<ActionPick>(1);
    let tools: Vec<Box<dyn rig::tool::ToolDyn>> = vec![
        Box::new(GeneratePickTool { tx: tx.clone() }),
        Box::new(AttackPickTool { tx: tx.clone() }),
        Box::new(RecoverPickTool { tx: tx.clone() }),
        Box::new(ReflectPickTool { tx: tx.clone() }),
        Box::new(AdaptPickTool { tx: tx.clone() }),
    ];
    drop(tx);

    let model = YazgModel::new(llm);
    let preamble = "\
You are AgenticAttackExecutionAgent. Call exactly one tool for the next step, \
or reply with plain text (no tool) to finish. Never invent HTTP results.";
    let agent = AgentBuilder::new(model)
        .name("AgenticAttackExecution")
        .preamble(preamble)
        .temperature(0.1)
        .max_tokens(256)
        .default_max_turns(PICK_MAX_TURNS)
        .tools(tools)
        .build();

    let goal = format!(
        "{transcript}\nOrchestrator step {step}/{max_steps}. Call one tool or finish with plain text."
    );
    let prompt_fut = agent.prompt(goal).max_turns(PICK_MAX_TURNS).into_future();
    await_first_pick(rx, prompt_fut).await
}

pick_tool!(
    SeqGeneratePickTool,
    "generate",
    AttackPickAction::Generate,
    "Generate payloads for the sequential attack attempt."
);
pick_tool!(
    SeqAttackPickTool,
    "attack",
    AttackPickAction::Attack,
    "Run HTTP attack+judge. Requires generate first."
);
pick_tool!(
    SeqRecoverPickTool,
    "recover",
    AttackPickAction::Recover,
    "Adjust endpoint pacing after an unhealthy attack observation."
);

/// Ask the orchestrator LLM to pick the next sequential action.
pub async fn pick_sequential_action(
    llm: Arc<dyn PlannerLlm>,
    transcript: &str,
    step: usize,
    max_steps: usize,
) -> Result<(String, AttackPickAction), String> {
    let (tx, rx) = mpsc::channel::<ActionPick>(1);
    let tools: Vec<Box<dyn rig::tool::ToolDyn>> = vec![
        Box::new(SeqGeneratePickTool { tx: tx.clone() }),
        Box::new(SeqAttackPickTool { tx: tx.clone() }),
        Box::new(SeqRecoverPickTool { tx: tx.clone() }),
    ];
    drop(tx);

    let model = YazgModel::new(llm);
    let preamble = "\
You are SequentialAttackExecutionAgent. Call exactly one tool for the next step, \
or reply with plain text (no tool) to finish.";
    let agent = AgentBuilder::new(model)
        .name("SequentialAttackExecution")
        .preamble(preamble)
        .temperature(0.1)
        .max_tokens(256)
        .default_max_turns(PICK_MAX_TURNS)
        .tools(tools)
        .build();

    let goal = format!(
        "{transcript}\nOrchestrator step {step}/{max_steps}. Call one tool or finish with plain text."
    );
    let prompt_fut = agent.prompt(goal).max_turns(PICK_MAX_TURNS).into_future();
    await_first_pick(rx, prompt_fut).await
}
