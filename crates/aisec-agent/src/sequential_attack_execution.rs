//! SequentialAttackExecutionAgent — ReAct generate → attack → recover → finish.
//!
//! Used for Sequential execution strategy. No reflection/adapt loop, but still
//! recovers from endpoint/transport failures via pacing adjustments.

use aisec_planner::PlannerLlm;
use serde::Deserialize;
use tracing::{info, warn};

use crate::attack_execution::{
    AttackAttemptObservation, AttackExecutionOutcome, AttackExecutionTools,
};
use crate::endpoint_recovery::{
    heuristic_recovery, observation_needs_recovery, MAX_ENDPOINT_RECOVERIES,
};
use crate::error::{AgentError, AgentResult};
use crate::memory::{
    load_memory_prompt_block, remember_ltm, remember_stm, AgentMemoryStore, LtmWrite,
    MemoryContext, MemoryScopeType, StmRole, StmWrite,
};
use crate::types::{AgentEvent, AgentId};

const DEFAULT_MAX_REACT_STEPS: usize = 24;

/// Request to execute one category sequentially (single logical attempt + recoveries).
#[derive(Debug, Clone)]
pub struct SequentialAttackExecutionRequest {
    pub category: String,
    pub max_react_steps: usize,
}

impl Default for SequentialAttackExecutionRequest {
    fn default() -> Self {
        Self {
            category: String::new(),
            max_react_steps: DEFAULT_MAX_REACT_STEPS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeqAction {
    Generate,
    Attack,
    Recover,
    Finish,
}

#[derive(Debug, Deserialize)]
struct SeqReactStep {
    thought: Option<String>,
    action: String,
}

/// Sequential scan orchestrator under Yazg.
pub struct SequentialAttackExecutionAgent;

impl SequentialAttackExecutionAgent {
    /// ReAct: generate → attack; on endpoint failure → recover (pacing) → re-attack → finish.
    pub async fn run(
        request: &SequentialAttackExecutionRequest,
        tools: &dyn AttackExecutionTools,
        orchestrator: Option<&dyn PlannerLlm>,
        llm_ready: bool,
        memory: Option<&dyn AgentMemoryStore>,
        memory_ctx: MemoryContext,
    ) -> AgentResult<AttackExecutionOutcome> {
        if request.category.trim().is_empty() {
            return Err(AgentError::AttackExecution(
                "SequentialAttackExecutionAgent requires a category".into(),
            ));
        }

        let max_steps = request.max_react_steps.max(8);
        let mut events = vec![AgentEvent::started(
            AgentId::SequentialAttackExecution,
            format!(
                "Sequential execution for {} (endpoint recovery enabled)",
                request.category
            ),
        )];

        info!(
            category = %request.category,
            "SequentialAttackExecutionAgent started"
        );

        remember_stm(
            memory,
            &memory_ctx,
            StmWrite {
                agent_id: AgentId::SequentialAttackExecution,
                role: StmRole::System,
                memory_key: Some("start".into()),
                content: format!("Start sequential category={}", request.category),
                content_json: None,
                importance: 0.7,
            },
        )
        .await;

        let memory_block =
            load_memory_prompt_block(memory, &memory_ctx, AgentId::SequentialAttackExecution)
                .await;

        let attempt = 1u32;
        let mut last_obs = AttackAttemptObservation::default();
        let mut generated = false;
        let mut attacked = false;
        let mut recoveries_used: u32 = 0;
        let mut needs_recover = false;
        let mut stopped_reason = "completed".to_string();

        let mut transcript = String::new();
        transcript.push_str(&format!(
            "You are SequentialAttackExecutionAgent running an authorized sequential AI security scan.\n\
             Category: {}\n\
             One logical attack attempt; use recover when the endpoint fails.\n\n",
            request.category
        ));
        if !memory_block.is_empty() {
            transcript.push_str(&memory_block);
        }
        transcript.push_str(
            "Respond with one JSON step each turn:\n\
             {\"thought\":\"...\",\"action\":\"generate|attack|recover|finish\"}\n\n\
             Policy:\n\
             - generate once, then attack.\n\
             - If attack errors or observation is unhealthy (timeouts, 429, 5xx, high latency),\n\
               recover: lower concurrency / serial wait / raise delay from response latency /\n\
               raise timeout / backoff, then attack again with the same payloads.\n\
             - finish after a healthy attack (or when recoveries are exhausted).\n\
             - Never invent HTTP results.\n",
        );

        for step in 1..=max_steps {
            if tools.is_cancelled() {
                stopped_reason = "cancelled".into();
                break;
            }
            tools.wait_if_paused().await;
            if tools.is_cancelled() {
                stopped_reason = "cancelled".into();
                break;
            }

            let action = if llm_ready {
                if let Some(llm) = orchestrator {
                    match decide_action(llm, &transcript, step, max_steps).await {
                        Ok((thought, action)) => {
                            events.push(AgentEvent::info(
                                AgentId::SequentialAttackExecution,
                                format!("Thought: {thought}"),
                            ));
                            transcript.push_str(&format!(
                                "\n--- Step {step} ---\nThought: {thought}\nAction: {action:?}\n"
                            ));
                            action
                        }
                        Err(err) => {
                            warn!(
                                error = %err,
                                "SequentialAttackExecutionAgent ReAct parse failed; using policy"
                            );
                            policy_next_action(
                                generated,
                                attacked,
                                needs_recover,
                                recoveries_used,
                                &last_obs,
                            )
                        }
                    }
                } else {
                    policy_next_action(
                        generated,
                        attacked,
                        needs_recover,
                        recoveries_used,
                        &last_obs,
                    )
                }
            } else {
                policy_next_action(
                    generated,
                    attacked,
                    needs_recover,
                    recoveries_used,
                    &last_obs,
                )
            };
            // Hard-gate: never skip endpoint recovery when the last attack was unhealthy.
            let action = if needs_recover && recoveries_used < MAX_ENDPOINT_RECOVERIES {
                SeqAction::Recover
            } else {
                action
            };

            events.push(AgentEvent::info(
                AgentId::SequentialAttackExecution,
                format!("Action: {action:?}"),
            ));

            match action {
                SeqAction::Generate => {
                    tools.set_phase("generate", attempt, 0).await;
                    tools
                        .emit_info(format!(
                            "SequentialAttackExecutionAgent: generating payloads for {}",
                            request.category
                        ))
                        .await;
                    tools.generate_payloads(attempt, &[]).await.map_err(|err| {
                        AgentError::AttackExecution(format!("sequential generate failed: {err}"))
                    })?;
                    tools.bump_progress(1).await;
                    generated = true;
                    attacked = false;
                    let obs = format!("generate ok attempt={attempt}");
                    transcript.push_str(&format!("Observation: {obs}\n"));
                    events.push(AgentEvent::info(
                        AgentId::SequentialAttackExecution,
                        obs.clone(),
                    ));
                    remember_stm(
                        memory,
                        &memory_ctx,
                        StmWrite {
                            agent_id: AgentId::SequentialAttackExecution,
                            role: StmRole::Observation,
                            memory_key: Some("generate".into()),
                            content: obs,
                            content_json: None,
                            importance: 0.5,
                        },
                    )
                    .await;
                }
                SeqAction::Attack => {
                    if !generated {
                        tools.set_phase("generate", attempt, 0).await;
                        tools.generate_payloads(attempt, &[]).await.map_err(|err| {
                            AgentError::AttackExecution(format!(
                                "sequential generate before attack: {err}"
                            ))
                        })?;
                        tools.bump_progress(1).await;
                        generated = true;
                    }
                    tools.set_phase("attack", attempt, recoveries_used).await;
                    match tools.run_attack_attempt(attempt).await {
                        Ok(obs) => {
                            tools.set_phase("judge", attempt, recoveries_used).await;
                            last_obs = obs;
                            attacked = true;
                            needs_recover = observation_needs_recovery(&last_obs)
                                && recoveries_used < MAX_ENDPOINT_RECOVERIES;
                            let line = format!(
                                "attack ok attempt={attempt} successes={} attempts={} high_confidence={} {}",
                                last_obs.successes,
                                last_obs.attempts,
                                last_obs.high_confidence_vuln,
                                last_obs.health_line()
                            );
                            transcript.push_str(&format!(
                                "Observation: {line}\nSummary: {}\nNeeds recover: {needs_recover}\n",
                                last_obs.summary
                            ));
                            events.push(AgentEvent::info(
                                AgentId::SequentialAttackExecution,
                                line.clone(),
                            ));
                            remember_stm(
                                memory,
                                &memory_ctx,
                                StmWrite {
                                    agent_id: AgentId::SequentialAttackExecution,
                                    role: StmRole::Observation,
                                    memory_key: Some("attack".into()),
                                    content: format!("{line} | {}", last_obs.summary),
                                    content_json: None,
                                    importance: 0.65,
                                },
                            )
                            .await;
                        }
                        Err(err) => {
                            last_obs.endpoint_error = Some(err.clone());
                            last_obs.endpoint_unhealthy = true;
                            attacked = false;
                            needs_recover = recoveries_used < MAX_ENDPOINT_RECOVERIES;
                            let line = format!("attack failed attempt={attempt}: {err}");
                            transcript.push_str(&format!(
                                "Observation: {line}\nNeeds recover: {needs_recover}\n"
                            ));
                            events.push(AgentEvent::info(
                                AgentId::SequentialAttackExecution,
                                line.clone(),
                            ));
                            remember_stm(
                                memory,
                                &memory_ctx,
                                StmWrite {
                                    agent_id: AgentId::SequentialAttackExecution,
                                    role: StmRole::Observation,
                                    memory_key: Some("attack_error".into()),
                                    content: line,
                                    content_json: None,
                                    importance: 0.75,
                                },
                            )
                            .await;
                            if !needs_recover {
                                stopped_reason = format!("attack_failed: {err}");
                                events.push(AgentEvent::failed(
                                    AgentId::SequentialAttackExecution,
                                    stopped_reason.clone(),
                                ));
                                return Err(AgentError::AttackExecution(stopped_reason));
                            }
                        }
                    }
                }
                SeqAction::Recover => {
                    if recoveries_used >= MAX_ENDPOINT_RECOVERIES {
                        needs_recover = false;
                        transcript.push_str(
                            "Observation: recover skipped — max endpoint recoveries reached\n",
                        );
                        continue;
                    }
                    tools
                        .set_phase("recover", attempt, recoveries_used)
                        .await;
                    let current = tools.current_pacing().await;
                    let plan = heuristic_recovery(&last_obs, &current, recoveries_used);
                    tools
                        .emit_info(format!(
                            "SequentialAttackExecutionAgent: recovering endpoint pacing — {}",
                            plan.summary()
                        ))
                        .await;
                    tools.apply_pacing(&plan.pacing).await.map_err(|err| {
                        AgentError::AttackExecution(format!("sequential recover failed: {err}"))
                    })?;
                    tools.wait_backoff(plan.wait_before_retry_ms).await;
                    recoveries_used = recoveries_used.saturating_add(1);
                    needs_recover = false;
                    attacked = false;
                    tools.bump_progress(1).await;
                    let line = format!(
                        "recover#{recoveries_used} applied: {}",
                        plan.summary()
                    );
                    transcript.push_str(&format!("Observation: {line}\n"));
                    events.push(AgentEvent::info(
                        AgentId::SequentialAttackExecution,
                        line.clone(),
                    ));
                    remember_stm(
                        memory,
                        &memory_ctx,
                        StmWrite {
                            agent_id: AgentId::SequentialAttackExecution,
                            role: StmRole::Observation,
                            memory_key: Some("recover".into()),
                            content: line,
                            content_json: Some(serde_json::json!({
                                "recoveries_used": recoveries_used,
                                "pacing": plan.pacing.summary(),
                                "wait_ms": plan.wait_before_retry_ms,
                                "notes": plan.notes,
                            })),
                            importance: 0.8,
                        },
                    )
                    .await;
                }
                SeqAction::Finish => {
                    stopped_reason = if last_obs.high_confidence_vuln {
                        "vulnerability confirmed".into()
                    } else if last_obs.any_vulnerable {
                        "finding recorded".into()
                    } else if last_obs.endpoint_unhealthy
                        || last_obs.endpoint_error.is_some()
                    {
                        format!(
                            "completed with endpoint issues after {recoveries_used} recover(ies)"
                        )
                    } else {
                        "completed".into()
                    };
                    break;
                }
            }

            if attacked && !needs_recover && !observation_needs_recovery(&last_obs) {
                stopped_reason = if last_obs.high_confidence_vuln {
                    "vulnerability confirmed".into()
                } else if last_obs.any_vulnerable {
                    "finding recorded".into()
                } else {
                    "completed".into()
                };
                break;
            }
            if attacked
                && observation_needs_recovery(&last_obs)
                && recoveries_used >= MAX_ENDPOINT_RECOVERIES
            {
                stopped_reason = format!(
                    "completed with endpoint issues after {recoveries_used} recover(ies)"
                );
                break;
            }
        }

        events.push(AgentEvent::completed(
            AgentId::SequentialAttackExecution,
            format!("{} done: {stopped_reason}", request.category),
        ));

        remember_stm(
            memory,
            &memory_ctx,
            StmWrite {
                agent_id: AgentId::SequentialAttackExecution,
                role: StmRole::Assistant,
                memory_key: Some("finish".into()),
                content: format!(
                    "category={} reason={} high_confidence={} recoveries={}",
                    request.category,
                    stopped_reason,
                    last_obs.high_confidence_vuln,
                    recoveries_used
                ),
                content_json: Some(serde_json::json!({
                    "category": request.category,
                    "stopped_reason": stopped_reason,
                    "any_vulnerable": last_obs.any_vulnerable,
                    "high_confidence_vuln": last_obs.high_confidence_vuln,
                    "recoveries_used": recoveries_used,
                    "endpoint_unhealthy": last_obs.endpoint_unhealthy,
                })),
                importance: 0.8,
            },
        )
        .await;

        let (scope_type, scope_id) = if let Some(scan_id) = memory_ctx.scan_id.as_ref() {
            (MemoryScopeType::Scan, scan_id.clone())
        } else {
            memory_ctx.primary_scope()
        };
        remember_ltm(
            memory,
            LtmWrite {
                agent_id: AgentId::SequentialAttackExecution,
                scope_type,
                scope_id,
                memory_key: format!("attack.{}.last_outcome", request.category),
                content: format!(
                    "sequential reason={} vulnerable={} recoveries={}",
                    stopped_reason, last_obs.any_vulnerable, recoveries_used
                ),
                content_json: Some(serde_json::json!({
                    "mode": "sequential",
                    "category": request.category,
                    "stopped_reason": stopped_reason,
                    "any_vulnerable": last_obs.any_vulnerable,
                    "high_confidence_vuln": last_obs.high_confidence_vuln,
                    "recoveries_used": recoveries_used,
                    "summary": last_obs.summary,
                    "health": last_obs.health_line(),
                })),
                importance: if last_obs.high_confidence_vuln {
                    0.95
                } else {
                    0.7
                },
            },
        )
        .await;

        Ok(AttackExecutionOutcome {
            category: request.category.clone(),
            attempts_run: attempt,
            last_observation: last_obs,
            stopped_reason,
            events,
        })
    }
}

fn policy_next_action(
    generated: bool,
    attacked: bool,
    needs_recover: bool,
    recoveries_used: u32,
    last_obs: &AttackAttemptObservation,
) -> SeqAction {
    if needs_recover && recoveries_used < MAX_ENDPOINT_RECOVERIES {
        return SeqAction::Recover;
    }
    if !generated {
        return SeqAction::Generate;
    }
    if !attacked {
        return SeqAction::Attack;
    }
    if observation_needs_recovery(last_obs) && recoveries_used < MAX_ENDPOINT_RECOVERIES {
        return SeqAction::Recover;
    }
    SeqAction::Finish
}

async fn decide_action(
    llm: &dyn PlannerLlm,
    transcript: &str,
    step: usize,
    max_steps: usize,
) -> Result<(String, SeqAction), String> {
    let prompt = format!(
        "{transcript}\nReAct step {step}/{max_steps}. Reply with one JSON object only.\n"
    );
    let raw = llm.complete(&prompt).await.map_err(|e| e.to_string())?;
    let parsed: SeqReactStep = {
        let json = extract_json_object(&raw)
            .ok_or_else(|| "no JSON in SequentialAttackExecutionAgent ReAct response".to_string())?;
        serde_json::from_str(&json).map_err(|e| e.to_string())?
    };
    let thought = parsed
        .thought
        .unwrap_or_else(|| "(no thought)".into())
        .trim()
        .to_string();
    let action = parse_seq_action(&parsed.action)?;
    Ok((thought, action))
}

fn parse_seq_action(raw: &str) -> Result<SeqAction, String> {
    match raw.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "generate" | "generate_payloads" | "payloads" => Ok(SeqAction::Generate),
        "attack" | "run_attack" | "execute" => Ok(SeqAction::Attack),
        "recover" | "recovery" | "pace" | "backoff" | "throttle" => Ok(SeqAction::Recover),
        "finish" | "done" | "stop" => Ok(SeqAction::Finish),
        other => Err(format!(
            "unknown SequentialAttackExecutionAgent action '{other}'"
        )),
    }
}

fn extract_json_object(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let start = trimmed.find('{')?;
    let slice = &trimmed[start..];
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    for (idx, ch) in slice.char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            if ch == '\\' {
                escape = true;
                continue;
            }
            if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(slice[..=idx].to_string());
                }
            }
            _ => {}
        }
    }
    None
}
