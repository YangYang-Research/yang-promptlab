//! SequentialAttackExecutionAgent — tool-calling generate → attack → recover → finish.
//!
//! Used for Sequential execution strategy. No reflection/adapt loop, but still
//! recovers from endpoint/transport failures via pacing adjustments.

use std::sync::Arc;

use promptlab_planner::PlannerLlm;
use tracing::{info, warn};

use crate::attack_execution::{
    emit_and_record, AttackAttemptObservation, AttackExecutionOutcome, AttackExecutionTools,
};
use crate::attack_execution_pick::{pick_sequential_action, AttackPickAction};
use crate::endpoint_recovery::{
    error_is_endpoint_recoverable, heuristic_recovery, observation_needs_recovery,
    seed_pacing_from_prior_failure, MAX_ENDPOINT_RECOVERIES,
};
use crate::error::{AgentError, AgentResult};
use crate::memory::{
    load_memory_prompt_block, load_prior_attack_failure_block, remember_attack_category_outcome,
    remember_stm, AgentMemoryStore, MemoryContext, StmRole, StmWrite,
};
use crate::types::{AgentEvent, AgentId};

const DEFAULT_MAX_REACT_STEPS: usize = 24;

/// Request to execute one category sequentially (single logical attempt + recoveries).
#[derive(Debug, Clone)]
pub struct SequentialAttackExecutionRequest {
    pub category: String,
    pub max_tool_turns: usize,
}

impl Default for SequentialAttackExecutionRequest {
    fn default() -> Self {
        Self {
            category: String::new(),
            max_tool_turns: DEFAULT_MAX_REACT_STEPS,
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

/// Sequential scan orchestrator under Yazg.
pub struct SequentialAttackExecutionAgent;

impl SequentialAttackExecutionAgent {
    /// Rig tools: generate → attack; on endpoint failure → recover → re-attack → finish.
    pub async fn run(
        request: &SequentialAttackExecutionRequest,
        tools: &dyn AttackExecutionTools,
        orchestrator: Option<Arc<dyn PlannerLlm>>,
        llm_ready: bool,
        memory: Option<&dyn AgentMemoryStore>,
        memory_ctx: MemoryContext,
    ) -> AgentResult<AttackExecutionOutcome> {
        if request.category.trim().is_empty() {
            return Err(AgentError::AttackExecution(
                "SequentialAttackExecutionAgent requires a category".into(),
            ));
        }

        let max_steps = request.max_tool_turns.max(8);
        let mut events = Vec::new();
        emit_and_record(
            tools,
            &mut events,
            AgentEvent::started(
                AgentId::SequentialAttackExecution,
                format!(
                    "Sequential execution for {} (endpoint recovery enabled)",
                    request.category
                ),
            ),
        )
        .await;

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

        let memory_block = load_memory_prompt_block(
            memory,
            &memory_ctx,
            AgentId::SequentialAttackExecution,
            Some(request.category.as_str()),
        )
        .await;
        let prior_failure = load_prior_attack_failure_block(
            memory,
            &memory_ctx,
            AgentId::SequentialAttackExecution,
            &request.category,
        )
        .await;
        if !prior_failure.is_empty() {
            remember_stm(
                memory,
                &memory_ctx,
                StmWrite {
                    agent_id: AgentId::SequentialAttackExecution,
                    role: StmRole::System,
                    memory_key: Some("prior_failure".into()),
                    content: prior_failure.trim().to_string(),
                    content_json: None,
                    importance: 0.95,
                },
            )
            .await;
            tools
                .emit_info(format!(
                    "SequentialAttackExecutionAgent: loaded prior failure context for {}",
                    request.category
                ))
                .await;
            let current = tools.current_pacing().await;
            // Host may already have inherited escalated pacing (auto-retry / prior category).
            // Never downgrade back to the mild prior-failure seed.
            if current.is_default() {
                let seed = seed_pacing_from_prior_failure(&current);
                tools.apply_pacing(&seed.pacing).await.map_err(|err| {
                    AgentError::AttackExecution(format!("sequential seed pacing failed: {err}"))
                })?;
                tools
                    .emit_info(format!(
                        "SequentialAttackExecutionAgent: seeded initial pacing from prior failure — {}",
                        seed.summary()
                    ))
                    .await;
            } else {
                tools
                    .emit_info(format!(
                        "SequentialAttackExecutionAgent: keeping inherited pacing — {}",
                        current.summary()
                    ))
                    .await;
            }
        }

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
        if !prior_failure.is_empty() {
            transcript.push_str(&prior_failure);
            transcript.push_str(
                "Prior failure pacing is already seeded. Always generate then attack first.\n\
                 Call recover only after an unhealthy attack observation in THIS run.\n\
                 Ignore failures from other categories.\n\n",
            );
        }
        transcript.push_str(
            "Call exactly one tool per turn. Available tools: generate, attack, recover.\n\
             When finished, reply with plain text (no tool call).\n\n\
             Policy:\n\
             - Only those three tools. Never use Yazg/Attack-Factory verbs \
               (generate_prompt, analyze_endpoint, attack_plan, recommend, …).\n\
             - generate once, then attack.\n\
             - If attack errors or observation is unhealthy (timeouts, 429, 5xx, or\n\
               no HTTP successes with extreme latency),\n\
               recover: lower concurrency / serial wait / raise delay from response latency /\n\
               raise timeout / backoff, then attack again with the same payloads.\n\
             - Do not recover solely because successful responses were slow.\n\
             - Never recover before the first attack observation; never recover twice without a new unhealthy attack.\n\
             - Stop with plain text only after at least one attack observation (healthy, or recoveries exhausted).\n\
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
                if let Some(llm) = orchestrator.as_ref() {
                    match pick_sequential_action(llm.clone(), &transcript, step, max_steps).await
                    {
                        Ok((thought, pick_action)) => {
                            let action = map_seq_pick_action(pick_action);
                            emit_and_record(
                                tools,
                                &mut events,
                                AgentEvent::react(
                                    AgentId::SequentialAttackExecution,
                                    format!("Thought: {thought}"),
                                ),
                            )
                            .await;
                            transcript.push_str(&format!(
                                "\n--- Step {step} ---\nThought: {thought}\nAction: {action:?}\n"
                            ));
                            action
                        }
                        Err(err) => {
                            warn!(
                                error = %err,
                                "SequentialAttackExecutionAgent action pick failed; using policy"
                            );
                            transcript.push_str(&format!(
                                "\n--- Step {step} ---\nInvalid tool choice: {err}\n\
                                 Valid tools: generate|attack|recover. Using policy.\n"
                            ));
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
            // Hard-gate: recover only after an unhealthy observation; never Finish before attack.
            let action = gate_seq_action(
                action,
                generated,
                attacked,
                needs_recover,
                recoveries_used,
                &last_obs,
            );

            emit_and_record(
                tools,
                &mut events,
                AgentEvent::react(
                    AgentId::SequentialAttackExecution,
                    format!("Action: {action:?}"),
                ),
            )
            .await;

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
                    // Host already bumped the sequential "generate" pipeline unit after
                    // pre-generation — do not double-count here or Progress jumps to 75%
                    // with Est. requests still 0/N while Attack is in flight.
                    generated = true;
                    attacked = false;
                    let obs = format!("generate ok attempt={attempt}");
                    transcript.push_str(&format!("Observation: {obs}\n"));
                    emit_and_record(
                        tools,
                        &mut events,
                        AgentEvent::info(
                        AgentId::SequentialAttackExecution,
                        obs.clone(),
                    ),
                    )
                    .await;
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
                        generated = true;
                    }
                    tools.set_phase("attack", attempt, recoveries_used).await;
                    match tools.run_attack_attempt(attempt).await {
                        Ok(obs) if obs.produced_no_requests() => {
                            let err = "attack produced no requests (empty payload batch or executor skipped all)".to_string();
                            last_obs = obs;
                            last_obs.endpoint_error = Some(err.clone());
                            last_obs.endpoint_unhealthy = true;
                            attacked = false;
                            // Empty batch is not fixed by pacing recover — fail the category.
                            stopped_reason = format!("attack_failed: {err}");
                            emit_and_record(
                                tools,
                                &mut events,
                                AgentEvent::info(
                                AgentId::SequentialAttackExecution,
                                format!("attack failed attempt={attempt}: {err}"),
                            ),
                            )
                            .await;
                            emit_and_record(
                                tools,
                                &mut events,
                                AgentEvent::failed(
                                AgentId::SequentialAttackExecution,
                                stopped_reason.clone(),
                            ),
                            )
                            .await;
                            remember_attack_category_outcome(
                                memory,
                                &memory_ctx,
                                AgentId::SequentialAttackExecution,
                                &request.category,
                                &stopped_reason,
                                format!(
                                    "sequential fatal {stopped_reason} recoveries={} {}",
                                    recoveries_used,
                                    last_obs.health_line()
                                ),
                                serde_json::json!({
                                    "mode": "sequential",
                                    "category": request.category,
                                    "stopped_reason": stopped_reason,
                                    "recoveries_used": recoveries_used,
                                    "endpoint_unhealthy": true,
                                    "endpoint_error": err,
                                    "health": last_obs.health_line(),
                                }),
                                0.95,
                                true,
                                Some(err.as_str()),
                            )
                            .await;
                            return Err(AgentError::AttackExecution(stopped_reason));
                        }
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
                            emit_and_record(
                                tools,
                                &mut events,
                                AgentEvent::info(
                                AgentId::SequentialAttackExecution,
                                line.clone(),
                            ),
                            )
                            .await;
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
                            let empty_batch = err.contains("no requests")
                                || err.contains("no payloads")
                                || err.contains("empty payload");
                            needs_recover = !empty_batch
                                && error_is_endpoint_recoverable(&err)
                                && recoveries_used < MAX_ENDPOINT_RECOVERIES;
                            let line = format!("attack failed attempt={attempt}: {err}");
                            transcript.push_str(&format!(
                                "Observation: {line}\nNeeds recover: {needs_recover}\n"
                            ));
                            emit_and_record(
                                tools,
                                &mut events,
                                AgentEvent::info(
                                AgentId::SequentialAttackExecution,
                                line.clone(),
                            ),
                            )
                            .await;
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
                                emit_and_record(
                                    tools,
                                    &mut events,
                                    AgentEvent::failed(
                                    AgentId::SequentialAttackExecution,
                                    stopped_reason.clone(),
                                ),
                                )
                                .await;
                                remember_attack_category_outcome(
                                    memory,
                                    &memory_ctx,
                                    AgentId::SequentialAttackExecution,
                                    &request.category,
                                    &stopped_reason,
                                    format!(
                                        "sequential fatal {} recoveries={} {}",
                                        stopped_reason,
                                        recoveries_used,
                                        last_obs.health_line()
                                    ),
                                    serde_json::json!({
                                        "mode": "sequential",
                                        "category": request.category,
                                        "stopped_reason": stopped_reason,
                                        "recoveries_used": recoveries_used,
                                        "endpoint_unhealthy": true,
                                        "endpoint_error": err,
                                        "health": last_obs.health_line(),
                                    }),
                                    0.95,
                                    true,
                                    Some(err.as_str()),
                                )
                                .await;
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
                    // Recoveries are outside the planned sequential pipeline budget
                    // (preparing + generate + attack + judge). Bumping here fills the
                    // bar while Est. requests stay flat and Attack retries continue.
                    let line = format!(
                        "recover#{recoveries_used} applied: {}",
                        plan.summary()
                    );
                    transcript.push_str(&format!("Observation: {line}\n"));
                    emit_and_record(
                        tools,
                        &mut events,
                        AgentEvent::info(
                        AgentId::SequentialAttackExecution,
                        line.clone(),
                    ),
                    )
                    .await;
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

            if attacked
                && last_obs.attempts > 0
                && !needs_recover
                && !observation_needs_recovery(&last_obs)
            {
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
                && last_obs.attempts > 0
                && observation_needs_recovery(&last_obs)
                && recoveries_used >= MAX_ENDPOINT_RECOVERIES
            {
                stopped_reason = format!(
                    "completed with endpoint issues after {recoveries_used} recover(ies)"
                );
                break;
            }
        }

        if (!attacked || last_obs.attempts == 0) && stopped_reason != "cancelled" {
            let reason = last_obs
                .endpoint_error
                .as_ref()
                .map(|err| format!("attack_failed: {err}"))
                .unwrap_or_else(|| {
                    "sequential category produced no successful attack attempts".into()
                });
            emit_and_record(
                tools,
                &mut events,
                AgentEvent::failed(
                AgentId::SequentialAttackExecution,
                reason.clone(),
            ),
            )
            .await;
            remember_attack_category_outcome(
                memory,
                &memory_ctx,
                AgentId::SequentialAttackExecution,
                &request.category,
                &reason,
                format!(
                    "sequential fatal {reason} recoveries={} {}",
                    recoveries_used,
                    last_obs.health_line()
                ),
                serde_json::json!({
                    "mode": "sequential",
                    "category": request.category,
                    "stopped_reason": reason,
                    "recoveries_used": recoveries_used,
                    "endpoint_unhealthy": last_obs.endpoint_unhealthy,
                    "endpoint_error": last_obs.endpoint_error,
                    "health": last_obs.health_line(),
                }),
                0.95,
                true,
                last_obs.endpoint_error.as_deref(),
            )
            .await;
            return Err(AgentError::AttackExecution(reason));
        }

        emit_and_record(
            tools,
            &mut events,
            AgentEvent::completed(
            AgentId::SequentialAttackExecution,
            format!("{} done: {stopped_reason}", request.category),
        ),
        )
        .await;

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

        remember_attack_category_outcome(
            memory,
            &memory_ctx,
            AgentId::SequentialAttackExecution,
            &request.category,
            &stopped_reason,
            format!(
                "sequential reason={} vulnerable={} recoveries={} {}",
                stopped_reason,
                last_obs.any_vulnerable,
                recoveries_used,
                last_obs.health_line()
            ),
            serde_json::json!({
                "mode": "sequential",
                "category": request.category,
                "stopped_reason": stopped_reason,
                "any_vulnerable": last_obs.any_vulnerable,
                "high_confidence_vuln": last_obs.high_confidence_vuln,
                "recoveries_used": recoveries_used,
                "summary": last_obs.summary,
                "health": last_obs.health_line(),
                "endpoint_unhealthy": last_obs.endpoint_unhealthy,
                "endpoint_error": last_obs.endpoint_error,
                "http_successes": last_obs.http_successes,
                "transport_errors": last_obs.transport_errors,
                "rate_limited": last_obs.rate_limited,
                "server_errors": last_obs.server_errors,
                "avg_latency_ms": last_obs.avg_latency_ms,
                "max_latency_ms": last_obs.max_latency_ms,
            }),
            if last_obs.high_confidence_vuln {
                0.95
            } else {
                0.7
            },
            last_obs.endpoint_unhealthy,
            last_obs.endpoint_error.as_deref(),
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

fn gate_seq_action(
    action: SeqAction,
    generated: bool,
    attacked: bool,
    needs_recover: bool,
    recoveries_used: u32,
    last_obs: &AttackAttemptObservation,
) -> SeqAction {
    if last_obs.high_confidence_vuln && attacked {
        return SeqAction::Finish;
    }
    if needs_recover && recoveries_used < MAX_ENDPOINT_RECOVERIES {
        return SeqAction::Recover;
    }
    if matches!(action, SeqAction::Recover)
        || (matches!(action, SeqAction::Finish) && !attacked)
    {
        return policy_next_action(
            generated,
            attacked,
            needs_recover,
            recoveries_used,
            last_obs,
        );
    }
    action
}

fn map_seq_pick_action(action: AttackPickAction) -> SeqAction {
    match action {
        AttackPickAction::Generate => SeqAction::Generate,
        AttackPickAction::Attack => SeqAction::Attack,
        AttackPickAction::Recover => SeqAction::Recover,
        AttackPickAction::Reflect | AttackPickAction::Adapt | AttackPickAction::Finish => SeqAction::Finish,
    }
}


fn parse_seq_action(raw: &str) -> Result<SeqAction, String> {
    match raw.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        // generate_prompt is a Yazg/Attack-Factory verb; treat as payload generate here.
        "generate" | "generate_payloads" | "payloads" | "generate_prompt" | "prompt" => {
            Ok(SeqAction::Generate)
        }
        "attack" | "run_attack" | "execute" => Ok(SeqAction::Attack),
        "recover" | "recovery" | "pace" | "backoff" | "throttle" => Ok(SeqAction::Recover),
        "finish" | "done" | "stop" => Ok(SeqAction::Finish),
        other => Err(format!(
            "unknown SequentialAttackExecutionAgent action '{other}'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_seq_action_accepts_yazg_generate_alias() {
        assert_eq!(
            parse_seq_action("generate_prompt").unwrap(),
            SeqAction::Generate
        );
        assert_eq!(parse_seq_action("GENERATE").unwrap(), SeqAction::Generate);
        assert_eq!(parse_seq_action("attack").unwrap(), SeqAction::Attack);
        assert_eq!(parse_seq_action("finish").unwrap(), SeqAction::Finish);
    }

    #[test]
    fn parse_seq_action_rejects_unknown() {
        assert!(parse_seq_action("analyze_endpoint").is_err());
    }

    #[test]
    fn policy_never_finishes_before_attack() {
        let obs = AttackAttemptObservation::default();
        assert_eq!(
            policy_next_action(false, false, false, 0, &obs),
            SeqAction::Generate
        );
        assert_eq!(
            policy_next_action(true, false, false, 0, &obs),
            SeqAction::Attack
        );
    }

    #[test]
    fn speculative_recover_rewrites_to_generate() {
        let obs = AttackAttemptObservation::default();
        assert_eq!(
            gate_seq_action(SeqAction::Recover, false, false, false, 0, &obs),
            SeqAction::Generate
        );
        assert_eq!(
            gate_seq_action(SeqAction::Recover, true, false, false, 0, &obs),
            SeqAction::Attack
        );
        assert_eq!(
            gate_seq_action(SeqAction::Recover, true, true, true, 0, &obs),
            SeqAction::Recover
        );
    }

    #[test]
    fn empty_observation_is_produced_no_requests() {
        assert!(AttackAttemptObservation::default().produced_no_requests());
        assert!(!AttackAttemptObservation {
            attempts: 1,
            ..Default::default()
        }
        .produced_no_requests());
    }
}
