//! AgenticAttackExecutionAgent — ReAct orchestrator for agentic scan execution.
//!
//! Coordinates generate → attack (host HTTP) → reflect → adapt → retry/finish.
//! Judge runs inside the host attack tool (JudgeCoordinatorAgent).

use async_trait::async_trait;
use aisec_planner::PlannerLlm;
use serde::Deserialize;
use tracing::{info, warn};

use crate::attack_plan::{AdaptPlanOutcome, AdaptPlanRequest, AttackPlanAgent};
use crate::endpoint_recovery::{
    heuristic_recovery, observation_needs_recovery, EndpointPacing, MAX_ENDPOINT_RECOVERIES,
};
use crate::error::{AgentError, AgentResult};
use crate::memory::{
    load_memory_prompt_block, load_prior_attack_failure_block, remember_attack_category_outcome,
    remember_stm, AgentMemoryStore, MemoryContext, StmRole, StmWrite,
};
use crate::reflection::{ReflectionAgent, ReflectionOutcome, ReflectionRequest};
use crate::types::{AgentEvent, AgentId};

const DEFAULT_MAX_REACT_STEPS: usize = 48;

/// Observation returned by the host after one attack+judge attempt.
#[derive(Debug, Clone, Default)]
pub struct AttackAttemptObservation {
    pub successes: u64,
    pub attempts: u64,
    pub any_vulnerable: bool,
    pub high_confidence_vuln: bool,
    pub summary: String,
    /// HTTP 2xx/3xx count from the attempt batch.
    pub http_successes: u64,
    pub transport_errors: u64,
    pub rate_limited: u64,
    pub server_errors: u64,
    pub avg_latency_ms: u64,
    pub max_latency_ms: u64,
    /// Host-side signal that the endpoint looked unhealthy this attempt.
    pub endpoint_unhealthy: bool,
    /// Soft error text when the host attack tool failed (timeout/connection/etc).
    pub endpoint_error: Option<String>,
}

impl AttackAttemptObservation {
    pub fn health_line(&self) -> String {
        format!(
            "http_ok={} transport_err={} rate_limited={} server_err={} avg_lat_ms={} max_lat_ms={} unhealthy={} err={}",
            self.http_successes,
            self.transport_errors,
            self.rate_limited,
            self.server_errors,
            self.avg_latency_ms,
            self.max_latency_ms,
            self.endpoint_unhealthy,
            self.endpoint_error.as_deref().unwrap_or("-")
        )
    }
}

/// Host tools for deterministic HTTP attack + payload generation (no LLM).
#[async_trait]
pub trait AttackExecutionTools: Send + Sync {
    fn is_cancelled(&self) -> bool;
    async fn wait_if_paused(&self);
    async fn set_phase(&self, phase: &str, attempt: u32, retry: u32);
    async fn bump_progress(&self, delta: u64);
    async fn emit_info(&self, message: String);

    /// Build / regenerate payloads for the upcoming attempt.
    async fn generate_payloads(
        &self,
        attempt: u32,
        focus_hints: &[String],
    ) -> Result<(), String>;

    /// Send payloads via HTTP harness and judge responses (includes JudgeCoordinator).
    async fn run_attack_attempt(
        &self,
        attempt: u32,
    ) -> Result<AttackAttemptObservation, String>;

    /// Apply AttackPlanAgent adapt directives to plan/strategy state.
    async fn apply_adapt(&self, adapt: &AdaptPlanOutcome) -> Result<(), String>;

    /// Current outbound pacing (concurrency / delay / timeout).
    async fn current_pacing(&self) -> EndpointPacing;

    /// Apply recovered pacing before the next attack retry.
    async fn apply_pacing(&self, pacing: &EndpointPacing) -> Result<(), String>;

    /// Sleep before retrying an unhealthy endpoint (host-controlled).
    async fn wait_backoff(&self, delay_ms: u64);
}

/// LLMs used by AgenticAttackExecutionAgent and its sub-agents.
pub struct AttackExecutionLlms<'a> {
    /// Orchestrator ReAct brain (may be same as plan/reflection).
    pub orchestrator: &'a dyn PlannerLlm,
    pub reflection: &'a dyn PlannerLlm,
    pub plan: &'a dyn PlannerLlm,
    /// When false, reflection/adapt use deterministic fallbacks.
    pub llm_ready: bool,
}

/// Request to execute one category (or scan slice) agentically.
#[derive(Debug, Clone)]
pub struct AttackExecutionRequest {
    pub category: String,
    pub max_attempts: u32,
    pub reflection_enabled: bool,
    pub adaptive_planning: bool,
    pub mutation_level: String,
    pub generation_strategy: String,
    pub variants_per_test: u8,
    pub response_adaptation: bool,
    pub max_react_steps: usize,
}

impl Default for AttackExecutionRequest {
    fn default() -> Self {
        Self {
            category: String::new(),
            max_attempts: 1,
            reflection_enabled: true,
            adaptive_planning: true,
            mutation_level: "medium".into(),
            generation_strategy: "deterministic".into(),
            variants_per_test: 3,
            response_adaptation: false,
            max_react_steps: DEFAULT_MAX_REACT_STEPS,
        }
    }
}

/// Outcome of AgenticAttackExecutionAgent for one category.
#[derive(Debug, Clone)]
pub struct AttackExecutionOutcome {
    pub category: String,
    pub attempts_run: u32,
    pub last_observation: AttackAttemptObservation,
    pub stopped_reason: String,
    pub events: Vec<AgentEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecAction {
    Generate,
    Attack,
    Recover,
    Reflect,
    Adapt,
    Finish,
}

#[derive(Debug, Deserialize)]
struct ExecReactStep {
    thought: Option<String>,
    action: String,
}

/// Agentic scan orchestrator under Yazg.
pub struct AgenticAttackExecutionAgent;

impl AgenticAttackExecutionAgent {
    /// ReAct-orchestrate one category through generate/attack/reflect/adapt.
    pub async fn run(
        request: &AttackExecutionRequest,
        tools: &dyn AttackExecutionTools,
        llms: &AttackExecutionLlms<'_>,
        memory: Option<&dyn AgentMemoryStore>,
        memory_ctx: MemoryContext,
    ) -> AgentResult<AttackExecutionOutcome> {
        if request.category.trim().is_empty() {
            return Err(AgentError::InvalidInput(
                "AgenticAttackExecutionAgent requires a category".into(),
            ));
        }
        let max_attempts = request.max_attempts.max(1);
        let max_steps = request.max_react_steps.max(8);

        let mut events = vec![AgentEvent::started(
            AgentId::AgenticAttackExecution,
            format!(
                "Agentic execution for {} (max_attempts={max_attempts})",
                request.category
            ),
        )];

        info!(
            category = %request.category,
            max_attempts,
            "AgenticAttackExecutionAgent started"
        );

        remember_stm(
            memory,
            &memory_ctx,
            StmWrite {
                agent_id: AgentId::AgenticAttackExecution,
                role: StmRole::System,
                memory_key: Some("start".into()),
                content: format!(
                    "Start agentic category={} max_attempts={max_attempts}",
                    request.category
                ),
                content_json: None,
                importance: 0.7,
            },
        )
        .await;

        let memory_block =
            load_memory_prompt_block(memory, &memory_ctx, AgentId::AgenticAttackExecution).await;
        let prior_failure = load_prior_attack_failure_block(
            memory,
            &memory_ctx,
            AgentId::AgenticAttackExecution,
            &request.category,
        )
        .await;
        if !prior_failure.is_empty() {
            remember_stm(
                memory,
                &memory_ctx,
                StmWrite {
                    agent_id: AgentId::AgenticAttackExecution,
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
                    "AgenticAttackExecutionAgent: loaded prior failure context for {}",
                    request.category
                ))
                .await;
        }

        let mut attempt: u32 = 0;
        let mut last_obs = AttackAttemptObservation::default();
        let mut last_reflection: Option<ReflectionOutcome> = None;
        let mut focus_hints: Vec<String> = Vec::new();
        let mut mutation_level = request.mutation_level.clone();
        let mut generation_strategy = request.generation_strategy.clone();
        let mut variants_per_test = request.variants_per_test;
        let mut response_adaptation = request.response_adaptation;
        let mut stopped_reason = "completed".to_string();

        let mut transcript = String::new();
        transcript.push_str(&format!(
            "You are AgenticAttackExecutionAgent orchestrating an authorized agentic AI security scan.\n\
             Category: {}\nMax attempts: {max_attempts}\n\
             Reflection enabled: {}\nAdaptive planning: {}\n\n",
            request.category, request.reflection_enabled, request.adaptive_planning
        ));
        if !memory_block.is_empty() {
            transcript.push_str(&memory_block);
        }
        if !prior_failure.is_empty() {
            transcript.push_str(&prior_failure);
            transcript.push_str(
                "If prior failure context is present, prefer recover early (serial wait, higher delay,\n\
                 longer timeout) before repeating the same unhealthy attack pattern.\n\n",
            );
        }
        transcript.push_str(
            "Respond with one JSON step each turn:\n\
             {\"thought\":\"...\",\"action\":\"generate|attack|recover|reflect|adapt|finish\"}\n\n\
             Policy:\n\
             - Start with generate then attack for each attempt.\n\
             - If attack fails or observation shows endpoint unhealthy (timeouts, 429, 5xx, high latency),\n\
               call recover to adjust pacing (lower concurrency, add inter-request delay, serial wait,\n\
               raise timeout, backoff), then attack again with the same payloads.\n\
             - After a healthy attack, call reflect when reflection is enabled (else finish or adapt/retry).\n\
             - If reflect says retry and attempts remain, call adapt when adaptive_planning is on, then generate again.\n\
             - Call finish when done (confirmed finding, no retry, cancel, or max attempts).\n\
             - Never invent HTTP results — only use observations.\n",
        );

        let mut generated_for_attempt: Option<u32> = None;
        let mut attacked_for_attempt: Option<u32> = None;
        let mut reflected_for_attempt: Option<u32> = None;
        let mut adapted_after_attempt: Option<u32> = None;
        let mut recoveries_used: u32 = 0;
        let mut needs_recover = false;

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

            let action = if llms.llm_ready {
                match decide_action(llms.orchestrator, &transcript, step, max_steps).await {
                    Ok((thought, action)) => {
                        events.push(AgentEvent::info(
                            AgentId::AgenticAttackExecution,
                            format!("Thought: {thought}"),
                        ));
                        transcript.push_str(&format!(
                            "\n--- Step {step} ---\nThought: {thought}\nAction: {:?}\n",
                            action
                        ));
                        action
                    }
                    Err(err) => {
                        warn!(error = %err, "AgenticAttackExecutionAgent ReAct parse failed; using policy");
                        policy_next_action(
                            request,
                            attempt,
                            max_attempts,
                            generated_for_attempt,
                            attacked_for_attempt,
                            reflected_for_attempt,
                            adapted_after_attempt,
                            last_reflection.as_ref(),
                            needs_recover,
                            recoveries_used,
                            &last_obs,
                        )
                    }
                }
            } else {
                policy_next_action(
                    request,
                    attempt,
                    max_attempts,
                    generated_for_attempt,
                    attacked_for_attempt,
                    reflected_for_attempt,
                    adapted_after_attempt,
                    last_reflection.as_ref(),
                    needs_recover,
                    recoveries_used,
                    &last_obs,
                )
            };
            // Hard-gate: never skip endpoint recovery when the last attack was unhealthy.
            let action = if needs_recover && recoveries_used < MAX_ENDPOINT_RECOVERIES {
                ExecAction::Recover
            } else {
                action
            };

            events.push(AgentEvent::info(
                AgentId::AgenticAttackExecution,
                format!("Action: {action:?}"),
            ));

            match action {
                ExecAction::Generate => {
                    let next_attempt = if attacked_for_attempt == Some(attempt) && attempt > 0 {
                        attempt.saturating_add(1).min(max_attempts)
                    } else if attempt == 0 {
                        1
                    } else {
                        attempt
                    };
                    if next_attempt > max_attempts {
                        stopped_reason = "max_attempts".into();
                        break;
                    }
                    attempt = next_attempt;
                    let retry = attempt.saturating_sub(1);
                    tools.set_phase("generate", attempt, retry).await;
                    tools
                        .emit_info(format!(
                            "AgenticAttackExecutionAgent: generating payloads for {} (attempt {attempt}/{max_attempts})",
                            request.category
                        ))
                        .await;
                    match tools.generate_payloads(attempt, &focus_hints).await {
                        Ok(()) => {
                            tools.bump_progress(1).await;
                            generated_for_attempt = Some(attempt);
                            attacked_for_attempt = None;
                            reflected_for_attempt = None;
                            let obs = format!("generate ok attempt={attempt}");
                            transcript.push_str(&format!("Observation: {obs}\n"));
                            events.push(AgentEvent::info(AgentId::AgenticAttackExecution, obs.clone()));
                            remember_stm(
                                memory,
                                &memory_ctx,
                                StmWrite {
                                    agent_id: AgentId::AgenticAttackExecution,
                                    role: StmRole::Observation,
                                    memory_key: Some("generate".into()),
                                    content: obs,
                                    content_json: None,
                                    importance: 0.5,
                                },
                            )
                            .await;
                        }
                        Err(err) => {
                            stopped_reason = format!("generate_failed: {err}");
                            events.push(AgentEvent::failed(
                                AgentId::AgenticAttackExecution,
                                stopped_reason.clone(),
                            ));
                            return Err(AgentError::AttackExecution(stopped_reason));
                        }
                    }
                }
                ExecAction::Attack => {
                    if attempt == 0 {
                        attempt = 1;
                    }
                    if generated_for_attempt != Some(attempt) {
                        // Ensure payloads exist before attack.
                        tools.set_phase("generate", attempt, attempt.saturating_sub(1)).await;
                        tools.generate_payloads(attempt, &focus_hints).await.map_err(|e| {
                            AgentError::AttackExecution(format!("generate before attack: {e}"))
                        })?;
                        tools.bump_progress(1).await;
                        generated_for_attempt = Some(attempt);
                    }
                    let retry = attempt.saturating_sub(1);
                    tools.set_phase("attack", attempt, retry).await;
                    match tools.run_attack_attempt(attempt).await {
                        Ok(obs) => {
                            tools.set_phase("judge", attempt, retry).await;
                            last_obs = obs;
                            attacked_for_attempt = Some(attempt);
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
                            events.push(AgentEvent::info(AgentId::AgenticAttackExecution, line.clone()));
                            remember_stm(
                                memory,
                                &memory_ctx,
                                StmWrite {
                                    agent_id: AgentId::AgenticAttackExecution,
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
                            attacked_for_attempt = None;
                            needs_recover = recoveries_used < MAX_ENDPOINT_RECOVERIES;
                            let line = format!("attack failed attempt={attempt}: {err}");
                            transcript.push_str(&format!(
                                "Observation: {line}\nNeeds recover: {needs_recover}\n"
                            ));
                            events.push(AgentEvent::info(
                                AgentId::AgenticAttackExecution,
                                line.clone(),
                            ));
                            remember_stm(
                                memory,
                                &memory_ctx,
                                StmWrite {
                                    agent_id: AgentId::AgenticAttackExecution,
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
                                    AgentId::AgenticAttackExecution,
                                    stopped_reason.clone(),
                                ));
                                remember_attack_category_outcome(
                                    memory,
                                    &memory_ctx,
                                    AgentId::AgenticAttackExecution,
                                    &request.category,
                                    &stopped_reason,
                                    format!(
                                        "agentic fatal {} recoveries={} {}",
                                        stopped_reason,
                                        recoveries_used,
                                        last_obs.health_line()
                                    ),
                                    serde_json::json!({
                                        "mode": "agentic",
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
                ExecAction::Recover => {
                    if recoveries_used >= MAX_ENDPOINT_RECOVERIES {
                        needs_recover = false;
                        transcript.push_str(
                            "Observation: recover skipped — max endpoint recoveries reached\n",
                        );
                        continue;
                    }
                    let retry = attempt.saturating_sub(1);
                    tools.set_phase("recover", attempt, retry).await;
                    let current = tools.current_pacing().await;
                    let plan = heuristic_recovery(&last_obs, &current, recoveries_used);
                    tools
                        .emit_info(format!(
                            "AgenticAttackExecutionAgent: recovering endpoint pacing — {}",
                            plan.summary()
                        ))
                        .await;
                    if let Err(err) = tools.apply_pacing(&plan.pacing).await {
                        stopped_reason = format!("recover_failed: {err}");
                        events.push(AgentEvent::failed(
                            AgentId::AgenticAttackExecution,
                            stopped_reason.clone(),
                        ));
                        return Err(AgentError::AttackExecution(stopped_reason));
                    }
                    tools.wait_backoff(plan.wait_before_retry_ms).await;
                    recoveries_used = recoveries_used.saturating_add(1);
                    needs_recover = false;
                    attacked_for_attempt = None;
                    reflected_for_attempt = None;
                    tools.bump_progress(1).await;
                    let line = format!(
                        "recover#{recoveries_used} applied: {}",
                        plan.summary()
                    );
                    transcript.push_str(&format!("Observation: {line}\n"));
                    events.push(AgentEvent::info(AgentId::AgenticAttackExecution, line.clone()));
                    remember_stm(
                        memory,
                        &memory_ctx,
                        StmWrite {
                            agent_id: AgentId::AgenticAttackExecution,
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
                ExecAction::Reflect => {
                    if attacked_for_attempt != Some(attempt) {
                        transcript.push_str(
                            "Observation: reflect skipped — attack this attempt first\n",
                        );
                        continue;
                    }
                    let retry = attempt.saturating_sub(1);
                    if request.reflection_enabled {
                        tools.set_phase("reflection", attempt, retry).await;
                    }
                    let refl_req = ReflectionRequest {
                        category: request.category.clone(),
                        attempt,
                        max_attempts,
                        successes: last_obs.successes,
                        attempts: last_obs.attempts,
                        judged_summary: format!(
                            "high_confidence={} vulnerable={} | {}",
                            if last_obs.high_confidence_vuln {
                                "true"
                            } else {
                                "false"
                            },
                            last_obs.any_vulnerable,
                            last_obs.summary
                        ),
                    };
                    let reflection = if request.reflection_enabled && llms.llm_ready {
                        match ReflectionAgent::run(&refl_req, llms.reflection).await {
                            Ok(outcome) => outcome,
                            Err(err) => {
                                warn!(error = %err, "ReflectionAgent failed; heuristic fallback");
                                ReflectionAgent::fallback_heuristic(&refl_req)
                            }
                        }
                    } else if request.reflection_enabled {
                        ReflectionAgent::fallback_heuristic(&refl_req)
                    } else {
                        // Mirror prior host behavior without reflection LLM.
                        let should_retry = !last_obs.any_vulnerable && attempt < max_attempts;
                        ReflectionOutcome {
                            should_retry,
                            reason: if should_retry {
                                "Reflection disabled — retry while no vulnerability".into()
                            } else {
                                "Reflection disabled — stopping".into()
                            },
                            focus_hints: Vec::new(),
                            events: vec![AgentEvent::info(
                                AgentId::Reflection,
                                "Reflection disabled — heuristic gate only",
                            )],
                        }
                    };
                    if request.reflection_enabled {
                        tools.bump_progress(1).await;
                    }
                    events.extend(reflection.events.clone());
                    focus_hints = reflection.focus_hints.clone();
                    let line = format!(
                        "reflect should_retry={} — {}",
                        reflection.should_retry, reflection.reason
                    );
                    transcript.push_str(&format!("Observation: {line}\n"));
                    events.push(AgentEvent::info(AgentId::AgenticAttackExecution, line.clone()));
                    remember_stm(
                        memory,
                        &memory_ctx,
                        StmWrite {
                            agent_id: AgentId::Reflection,
                            role: StmRole::Observation,
                            memory_key: Some("reflect".into()),
                            content: line,
                            content_json: None,
                            importance: 0.7,
                        },
                    )
                    .await;
                    reflected_for_attempt = Some(attempt);
                    last_reflection = Some(reflection);
                }
                ExecAction::Adapt => {
                    let Some(ref reflection) = last_reflection else {
                        transcript.push_str("Observation: adapt skipped — reflect first\n");
                        continue;
                    };
                    if !reflection.should_retry || attempt >= max_attempts {
                        stopped_reason = reflection.reason.clone();
                        break;
                    }
                    if !request.adaptive_planning {
                        adapted_after_attempt = Some(attempt);
                        transcript.push_str(
                            "Observation: adaptive_planning disabled — proceed to generate\n",
                        );
                        continue;
                    }
                    tools
                        .set_phase("adaptive", attempt.saturating_add(1), attempt)
                        .await;
                    let adapt_req = AdaptPlanRequest {
                        category: request.category.clone(),
                        attempt,
                        max_attempts,
                        mutation_level: mutation_level.clone(),
                        generation_strategy: generation_strategy.clone(),
                        variants_per_test,
                        response_adaptation,
                        last_result_summary: last_obs.summary.clone(),
                        reflection_reason: Some(reflection.reason.clone()),
                        focus_hints: focus_hints.clone(),
                    };
                    let adapt = if llms.llm_ready {
                        match AttackPlanAgent::adapt(&adapt_req, llms.plan).await {
                            Ok(outcome) => outcome,
                            Err(err) => {
                                warn!(error = %err, "AttackPlanAgent adapt failed; fallback");
                                AttackPlanAgent::adapt_fallback(&adapt_req)
                            }
                        }
                    } else {
                        AttackPlanAgent::adapt_fallback(&adapt_req)
                    };
                    events.extend(adapt.events.clone());
                    if adapt.escalate_mutation {
                        mutation_level = format!("{mutation_level}+");
                    }
                    if adapt.escalate_strategy {
                        generation_strategy = format!("{generation_strategy}+");
                    }
                    if adapt.increase_variants {
                        variants_per_test = variants_per_test.saturating_add(2).min(20);
                    }
                    if adapt.enable_response_adaptation {
                        response_adaptation = true;
                    }
                    if let Err(err) = tools.apply_adapt(&adapt).await {
                        stopped_reason = format!("adapt_failed: {err}");
                        events.push(AgentEvent::failed(
                            AgentId::AgenticAttackExecution,
                            stopped_reason.clone(),
                        ));
                        return Err(AgentError::AttackExecution(stopped_reason));
                    }
                    tools.bump_progress(1).await;
                    tools.set_phase("retry", attempt.saturating_add(1), attempt).await;
                    tools.bump_progress(1).await;
                    adapted_after_attempt = Some(attempt);
                    let line = if adapt.notes.is_empty() {
                        "adapt applied".into()
                    } else {
                        format!("adapt applied: {}", adapt.notes.join("; "))
                    };
                    transcript.push_str(&format!("Observation: {line}\n"));
                    events.push(AgentEvent::info(AgentId::AgenticAttackExecution, line));
                }
                ExecAction::Finish => {
                    if let Some(ref reflection) = last_reflection {
                        stopped_reason = reflection.reason.clone();
                    } else if last_obs.any_vulnerable {
                        stopped_reason = "vulnerability confirmed".into();
                    } else if attempt >= max_attempts {
                        stopped_reason = "max_attempts".into();
                    } else {
                        stopped_reason = "finished".into();
                    }
                    break;
                }
            }

            // Auto-finish when reflection says stop and we've reflected.
            if let Some(ref reflection) = last_reflection {
                if reflected_for_attempt == Some(attempt) && !reflection.should_retry {
                    stopped_reason = reflection.reason.clone();
                    break;
                }
                if reflection.should_retry
                    && attempt >= max_attempts
                    && reflected_for_attempt == Some(attempt)
                {
                    stopped_reason = "max_attempts".into();
                    break;
                }
            }
        }

        events.push(AgentEvent::completed(
            AgentId::AgenticAttackExecution,
            format!(
                "{} done after {attempt} attempt(s): {stopped_reason}",
                request.category
            ),
        ));

        remember_stm(
            memory,
            &memory_ctx,
            StmWrite {
                agent_id: AgentId::AgenticAttackExecution,
                role: StmRole::Assistant,
                memory_key: Some("finish".into()),
                content: format!(
                    "category={} attempts={} reason={} high_confidence={}",
                    request.category, attempt, stopped_reason, last_obs.high_confidence_vuln
                ),
                content_json: Some(serde_json::json!({
                    "category": request.category,
                    "attempts": attempt,
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
            AgentId::AgenticAttackExecution,
            &request.category,
            &stopped_reason,
            format!(
                "attempts={} reason={} vulnerable={} recoveries={} {}",
                attempt,
                stopped_reason,
                last_obs.any_vulnerable,
                recoveries_used,
                last_obs.health_line()
            ),
            serde_json::json!({
                "mode": "agentic",
                "category": request.category,
                "attempts_run": attempt,
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
    request: &AttackExecutionRequest,
    attempt: u32,
    max_attempts: u32,
    generated_for_attempt: Option<u32>,
    attacked_for_attempt: Option<u32>,
    reflected_for_attempt: Option<u32>,
    adapted_after_attempt: Option<u32>,
    last_reflection: Option<&ReflectionOutcome>,
    needs_recover: bool,
    recoveries_used: u32,
    last_obs: &AttackAttemptObservation,
) -> ExecAction {
    if needs_recover && recoveries_used < MAX_ENDPOINT_RECOVERIES {
        return ExecAction::Recover;
    }
    if attempt == 0 {
        return ExecAction::Generate;
    }
    if generated_for_attempt != Some(attempt) {
        return ExecAction::Generate;
    }
    if attacked_for_attempt != Some(attempt) {
        return ExecAction::Attack;
    }
    if observation_needs_recovery(last_obs) && recoveries_used < MAX_ENDPOINT_RECOVERIES {
        return ExecAction::Recover;
    }
    if reflected_for_attempt != Some(attempt) {
        return ExecAction::Reflect;
    }
    if let Some(reflection) = last_reflection {
        if !reflection.should_retry {
            return ExecAction::Finish;
        }
        if attempt >= max_attempts {
            return ExecAction::Finish;
        }
        if request.adaptive_planning && adapted_after_attempt != Some(attempt) {
            return ExecAction::Adapt;
        }
        // After adapt (or if adapt disabled), start next attempt via generate.
        return ExecAction::Generate;
    }
    ExecAction::Finish
}

async fn decide_action(
    llm: &dyn PlannerLlm,
    transcript: &str,
    step: usize,
    max_steps: usize,
) -> Result<(String, ExecAction), String> {
    let prompt = format!(
        "{transcript}\nReAct step {step}/{max_steps}. Reply with one JSON object only.\n"
    );
    let raw = llm
        .complete(&prompt)
        .await
        .map_err(|e| e.to_string())?;
    let parsed: ExecReactStep = {
        let json = extract_json_object(&raw)
            .ok_or_else(|| "no JSON in AgenticAttackExecutionAgent ReAct response".to_string())?;
        serde_json::from_str(&json).map_err(|e| e.to_string())?
    };
    let thought = parsed
        .thought
        .unwrap_or_else(|| "(no thought)".into())
        .trim()
        .to_string();
    let action = parse_exec_action(&parsed.action)?;
    Ok((thought, action))
}

fn parse_exec_action(raw: &str) -> Result<ExecAction, String> {
    match raw.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "generate" | "generate_payloads" | "payloads" => Ok(ExecAction::Generate),
        "attack" | "run_attack" | "execute" => Ok(ExecAction::Attack),
        "recover" | "recovery" | "pace" | "backoff" | "throttle" => Ok(ExecAction::Recover),
        "reflect" | "reflection" => Ok(ExecAction::Reflect),
        "adapt" | "adaptive" | "replan" => Ok(ExecAction::Adapt),
        "finish" | "done" | "stop" => Ok(ExecAction::Finish),
        other => Err(format!("unknown AgenticAttackExecutionAgent action '{other}'")),
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
