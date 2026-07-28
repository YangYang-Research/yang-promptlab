//! Rig AgentBuilder decision helpers for scan orchestrators.
//!
//! Host tools (`AttackExecutionTools`) are borrowed (`&dyn`) from the Tauri scan
//! host and are therefore non-`'static`, so they cannot be registered directly on
//! a Rig Agent. The pick tools choose an action; the host loop executes against
//! `&dyn AttackExecutionTools` (same observation contract as a full in-agent tool).
//!
//! Yazg chat/wizard specialists use true manager–worker (`manager.tool(worker_agent)`).

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

use crate::rig_model::YazgRigModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackRigAction {
    Generate,
    Attack,
    Recover,
    Reflect,
    Adapt,
    Finish,
}

struct ActionPick {
    action: AttackRigAction,
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
    AttackRigAction::Generate,
    "Generate / regenerate payloads for the next attack attempt."
);
pick_tool!(
    AttackPickTool,
    "attack",
    AttackRigAction::Attack,
    "Run HTTP attack+judge for the current attempt."
);
pick_tool!(
    RecoverPickTool,
    "recover",
    AttackRigAction::Recover,
    "Adjust endpoint pacing after an unhealthy attack observation."
);
pick_tool!(
    ReflectPickTool,
    "reflect",
    AttackRigAction::Reflect,
    "Decide whether to retry after a healthy attack observation."
);
pick_tool!(
    AdaptPickTool,
    "adapt",
    AttackRigAction::Adapt,
    "Adapt payload strategy after reflect says retry."
);

/// Ask a Rig agent to pick the next agentic action (one tool call), or Finish on text.
pub async fn pick_agentic_action_rig(
    llm: Arc<dyn PlannerLlm>,
    transcript: &str,
    step: usize,
    max_steps: usize,
) -> Result<(String, AttackRigAction), String> {
    let (tx, mut rx) = mpsc::channel::<ActionPick>(1);
    let tools: Vec<Box<dyn rig::tool::ToolDyn>> = vec![
        Box::new(GeneratePickTool { tx: tx.clone() }),
        Box::new(AttackPickTool { tx: tx.clone() }),
        Box::new(RecoverPickTool { tx: tx.clone() }),
        Box::new(ReflectPickTool { tx: tx.clone() }),
        Box::new(AdaptPickTool { tx: tx.clone() }),
    ];
    drop(tx);

    let model = YazgRigModel::new(llm);
    let preamble = "\
You are AgenticAttackExecutionAgent. Call exactly one tool for the next step, \
or reply with plain text (no tool) to finish. Never invent HTTP results.";
    let agent = AgentBuilder::new(model)
        .name("AgenticAttackExecution")
        .preamble(preamble)
        .temperature(0.1)
        .max_tokens(256)
        .default_max_turns(2)
        .tools(tools)
        .build();

    let goal = format!(
        "{transcript}\nRig step {step}/{max_steps}. Call one tool or finish with plain text."
    );
    let prompt_fut = agent.prompt(goal).max_turns(2).into_future();
    tokio::pin!(prompt_fut);

    let mut picked: Option<AttackRigAction> = None;
    let mut thought = String::from("(rig)");
    loop {
        tokio::select! {
            biased;
            Some(pick) = rx.recv() => {
                picked = Some(pick.action);
                let _ = pick.reply.send("acknowledged — host will execute".into());
                // Drain/cancel remaining agent work by dropping — wait briefly for prompt.
                let _ = prompt_fut.await;
                break;
            }
            result = &mut prompt_fut => {
                match result {
                    Ok(text) => {
                        thought = text.trim().to_string();
                        if thought.is_empty() {
                            thought = "finish".into();
                        }
                    }
                    Err(err) => return Err(err.to_string()),
                }
                break;
            }
        }
    }

    Ok((
        thought,
        picked.unwrap_or(AttackRigAction::Finish),
    ))
}

pick_tool!(
    SeqGeneratePickTool,
    "generate",
    AttackRigAction::Generate,
    "Generate payloads for the sequential attack attempt."
);
pick_tool!(
    SeqAttackPickTool,
    "attack",
    AttackRigAction::Attack,
    "Run HTTP attack+judge. Requires generate first."
);
pick_tool!(
    SeqRecoverPickTool,
    "recover",
    AttackRigAction::Recover,
    "Adjust endpoint pacing after an unhealthy attack observation."
);

/// Ask a Rig agent to pick the next sequential action.
pub async fn pick_sequential_action_rig(
    llm: Arc<dyn PlannerLlm>,
    transcript: &str,
    step: usize,
    max_steps: usize,
) -> Result<(String, AttackRigAction), String> {
    let (tx, mut rx) = mpsc::channel::<ActionPick>(1);
    let tools: Vec<Box<dyn rig::tool::ToolDyn>> = vec![
        Box::new(SeqGeneratePickTool { tx: tx.clone() }),
        Box::new(SeqAttackPickTool { tx: tx.clone() }),
        Box::new(SeqRecoverPickTool { tx: tx.clone() }),
    ];
    drop(tx);

    let model = YazgRigModel::new(llm);
    let preamble = "\
You are SequentialAttackExecutionAgent. Call exactly one tool for the next step, \
or reply with plain text (no tool) to finish.";
    let agent = AgentBuilder::new(model)
        .name("SequentialAttackExecution")
        .preamble(preamble)
        .temperature(0.1)
        .max_tokens(256)
        .default_max_turns(2)
        .tools(tools)
        .build();

    let goal = format!(
        "{transcript}\nRig step {step}/{max_steps}. Call one tool or finish with plain text."
    );
    let prompt_fut = agent.prompt(goal).max_turns(2).into_future();
    tokio::pin!(prompt_fut);

    let mut picked: Option<AttackRigAction> = None;
    let mut thought = String::from("(rig)");
    loop {
        tokio::select! {
            biased;
            Some(pick) = rx.recv() => {
                picked = Some(pick.action);
                let _ = pick.reply.send("acknowledged — host will execute".into());
                let _ = prompt_fut.await;
                break;
            }
            result = &mut prompt_fut => {
                match result {
                    Ok(text) => {
                        thought = text.trim().to_string();
                        if thought.is_empty() {
                            thought = "finish".into();
                        }
                    }
                    Err(err) => return Err(err.to_string()),
                }
                break;
            }
        }
    }

    Ok((
        thought,
        picked.unwrap_or(AttackRigAction::Finish),
    ))
}
