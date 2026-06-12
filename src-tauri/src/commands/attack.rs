//! Attack execution commands.
//!
//! `attack_run_prompt_injection` executes the real `aisec-attack` prompt
//! injection attack (HTTP transport + built-in evaluation) against a previously
//! discovered endpoint, persists every attempt as an `attack_result` and every
//! successful attempt as a `finding`, and returns the run summary. No mocked
//! findings: results come straight from the engine evaluating real HTTP
//! responses.

use aisec_attack::{
    default_executor, AttackCategory, AttackContext, AttackTarget, FindingSeverity,
};
use aisec_judge::{deterministic_engine, JudgeRequest, JudgeVerdict, Severity as JudgeSeverity};
use aisec_storage::{
    AttackResultRepository, CreateAttackResult, CreateFinding, CreateScan, EndpointRepository,
    FindingRepository, ScanRepository, UpdateScan,
};
use tauri::State;
use time::OffsetDateTime;
use tracing::{info, instrument, warn};

use crate::dto::{AttackRunDto, FindingDto, ScanDto};
use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

fn severity_str(severity: FindingSeverity) -> &'static str {
    match severity {
        FindingSeverity::Info => "info",
        FindingSeverity::Low => "low",
        FindingSeverity::Medium => "medium",
        FindingSeverity::High => "high",
        FindingSeverity::Critical => "critical",
    }
}

fn judge_severity_str(severity: JudgeSeverity) -> &'static str {
    match severity {
        JudgeSeverity::Info => "info",
        JudgeSeverity::Low => "low",
        JudgeSeverity::Medium => "medium",
        JudgeSeverity::High => "high",
        JudgeSeverity::Critical => "critical",
    }
}

#[instrument(skip(state))]
pub async fn attack_run_prompt_injection_op(
    state: &AppState,
    endpoint_id: String,
) -> CommandResult<AttackRunDto> {
    let repos = state.repositories();

    let endpoint = repos.endpoints().get(&endpoint_id).await.map_err(CommandError::from)?;
    let source_scan = repos.scans().get(&endpoint.scan_id).await.map_err(CommandError::from)?;
    let project_id = source_scan.project_id.clone();
    let target_id = endpoint.target_id.clone().or(source_scan.target_id.clone());

    // Dedicated scan for this attack run.
    let scan = repos
        .scans()
        .create(CreateScan {
            project_id: project_id.clone(),
            target_id: target_id.clone(),
            name: format!("Prompt Injection: {}", endpoint.url),
            status: Some("running".into()),
            playbook_json: Some(serde_json::json!({
                "attack": "prompt_injection",
                "endpoint_id": endpoint.id,
                "endpoint_url": endpoint.url,
            })),
        })
        .await
        .map_err(CommandError::from)?;

    let _ = repos
        .scans()
        .update(
            &scan.id,
            UpdateScan {
                started_at: Some(Some(OffsetDateTime::now_utc())),
                ..Default::default()
            },
        )
        .await;

    // Execute the real attack engine over HTTP.
    let target = AttackTarget::llm_api(endpoint.url.clone());
    let ctx = AttackContext::new(scan.id.clone(), "probe-prompt-injection", target);
    let executor = default_executor();

    info!(scan_id = %scan.id, url = %endpoint.url, "prompt injection attack started");

    let result = match executor
        .execute_category(AttackCategory::PromptInjection, &ctx)
        .await
    {
        Ok(result) => result,
        Err(err) => {
            warn!(scan_id = %scan.id, error = %err, "attack run failed");
            let _ = repos
                .scans()
                .update(
                    &scan.id,
                    UpdateScan {
                        status: Some("failed".into()),
                        completed_at: Some(Some(OffsetDateTime::now_utc())),
                        playbook_json: Some(serde_json::json!({ "error": err.to_string() })),
                        ..Default::default()
                    },
                )
                .await;
            return Err(CommandError::from(aisec_core::AisecError::internal(
                err.to_string(),
            )));
        }
    };

    // Real evaluation of every attack response by the deterministic Judge
    // Engine (rule + regex consensus, no mocked verdicts).
    let judge = deterministic_engine();

    let mut successes = 0u64;
    let mut created_findings: Vec<FindingDto> = Vec::new();

    for attempt in &result.attempts {
        let eval = &attempt.evaluation;

        let verdict: JudgeVerdict = judge
            .judge_deterministic(JudgeRequest {
                probe_id: attempt.payload_id.clone(),
                attack_category: "prompt_injection".into(),
                payload: attempt.mutated_content.clone(),
                response_text: attempt.response.body.clone(),
                context: serde_json::json!({ "payload_name": attempt.payload_name }),
            })
            .await
            .map_err(|err| CommandError::from(aisec_core::AisecError::internal(err.to_string())))?;

        let judge_json = serde_json::to_value(&verdict).unwrap_or(serde_json::Value::Null);

        // Persist every attempt as an attack_result, including the judge verdict.
        let _ = repos
            .attack_results()
            .create(CreateAttackResult {
                scan_id: scan.id.clone(),
                payload_id: None,
                target_id: target_id.clone(),
                probe_id: Some(attempt.payload_id.clone()),
                success: verdict.vulnerable,
                response_json: Some(serde_json::json!({
                    "status": attempt.response.status,
                    "body": attempt.response.body,
                    "duration_ms": attempt.response.duration_ms,
                })),
                evaluated_json: Some(serde_json::json!({
                    "attack_evaluation": {
                        "success": eval.success,
                        "confidence": eval.confidence,
                        "severity": eval.severity.map(severity_str),
                        "indicators": eval.indicators,
                        "summary": eval.summary,
                    },
                    "judge": judge_json,
                })),
                duration_ms: Some(attempt.response.duration_ms as i64),
            })
            .await
            .map_err(CommandError::from)?;

        // The Judge Engine verdict is authoritative for findings.
        if verdict.vulnerable {
            successes += 1;
            let severity = verdict
                .severity
                .map(judge_severity_str)
                .or_else(|| eval.severity.map(severity_str))
                .unwrap_or("medium");
            let finding = repos
                .findings()
                .create(CreateFinding {
                    scan_id: scan.id.clone(),
                    project_id: project_id.clone(),
                    target_id: target_id.clone(),
                    title: format!("Prompt injection: {}", attempt.payload_name),
                    severity: severity.to_string(),
                    category: Some("prompt_injection".into()),
                    description: Some(verdict.summary.clone()),
                    evidence_json: Some(serde_json::json!({
                        "payload_id": attempt.payload_id,
                        "payload": attempt.mutated_content,
                        "verdict": "vulnerable",
                        "confidence": verdict.confidence,
                        "indicators": eval.indicators,
                        "response_excerpt": attempt.response.body.chars().take(500).collect::<String>(),
                        "judge": judge_json,
                    })),
                    status: None,
                })
                .await
                .map_err(CommandError::from)?;
            created_findings.push(FindingDto::from(finding));
        }
    }

    let updated = repos
        .scans()
        .update(
            &scan.id,
            UpdateScan {
                status: Some("completed".into()),
                completed_at: Some(Some(OffsetDateTime::now_utc())),
                ..Default::default()
            },
        )
        .await
        .map_err(CommandError::from)?;

    info!(
        scan_id = %scan.id,
        attempts = result.attempts.len(),
        successes,
        findings = created_findings.len(),
        "prompt injection attack completed"
    );

    Ok(AttackRunDto {
        scan: ScanDto::from(updated),
        category: "prompt_injection".into(),
        attempts: result.attempts.len() as u64,
        successes,
        findings: created_findings,
    })
}

// ---------------------------------------------------------------------------
// Tauri command wrapper
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn attack_run_prompt_injection(
    state: State<'_, AppState>,
    endpoint_id: String,
) -> CommandResult<AttackRunDto> {
    attack_run_prompt_injection_op(state.inner(), endpoint_id).await
}
