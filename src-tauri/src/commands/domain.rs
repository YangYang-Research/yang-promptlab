//! Domain IPC commands operating directly on SQLite via the repository layer.
//!
//! Each command is a thin `#[tauri::command]` wrapper over a testable `*_op`
//! function that takes `&AppState`, so the same logic is exercised by the
//! integration tests without a Tauri runtime.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use promptlab_auth::{
    resolve_descriptor_for_wizard, sanitize_target_descriptor, SecretStore,
};
use promptlab_core::PromptLabError;
use promptlab_report::{
    formatter_for, parse_sarif_import, stored_recommendations_from_playbook, GeneratedReport,
    ReportDataBuilder, ReportFormat, ReportKind, ReportingEngine, StorageFindingRow,
};
use promptlab_storage::{
    CreateFinding, CreateReport, CreateScan, CreateTarget, FindingRepository, ProjectRepository,
    ReportRepository, ScanRepository, TargetRepository, UpdateFinding,
};
use tauri::State;
use tracing::{info, instrument};

use crate::dto::{
    FindingDto, FindingImportDto, ReportContentDto, ReportDto, ScanDetailDto, ScanDto, TargetDto,
};
use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

fn map_report_err(err: promptlab_report::ReportError) -> CommandError {
    CommandError::from(PromptLabError::internal(err.to_string()))
}

fn parse_format(value: Option<&str>) -> ReportFormat {
    match value.map(str::to_ascii_lowercase).as_deref() {
        Some("pdf") => ReportFormat::Pdf,
        Some("json") => ReportFormat::Json,
        Some("sarif") => ReportFormat::Sarif,
        Some("csv") => ReportFormat::Csv,
        _ => ReportFormat::Html,
    }
}

fn parse_kind(value: Option<&str>) -> ReportKind {
    match value.map(str::to_ascii_lowercase).as_deref() {
        Some("executive") => ReportKind::Executive,
        Some("compliance") => ReportKind::Compliance,
        _ => ReportKind::Technical,
    }
}

async fn render_scan_report(
    state: &AppState,
    project_id: &str,
    scan_id: &str,
    format: Option<&str>,
    kind: Option<&str>,
) -> CommandResult<(GeneratedReport, usize)> {
    let repos = state.repositories();
    let project = repos.projects().get(project_id).await.map_err(CommandError::from)?;
    let scan = repos.scans().get(scan_id).await.map_err(CommandError::from)?;
    let findings = repos
        .findings()
        .list_by_scan(scan_id)
        .await
        .map_err(CommandError::from)?;

    let target_name = match &scan.target_id {
        Some(tid) => repos.targets().get(tid).await.ok().map(|t| t.name),
        None => None,
    };

    let rows: Vec<StorageFindingRow> = findings
        .iter()
        .map(|f| StorageFindingRow {
            id: f.id.clone(),
            title: f.title.clone(),
            severity: f.severity.clone(),
            category: f.category.clone(),
            description: f.description.clone(),
            evidence_json: f.evidence_json.clone(),
            status: f.status.clone(),
        })
        .collect();

    let report_findings = ReportDataBuilder::from_storage_findings(&rows);
    let mut input = ReportDataBuilder::with_context(
        ReportDataBuilder::build(
            scan_id.to_string(),
            project.name.clone(),
            target_name,
            report_findings,
        ),
        project_id.to_string(),
        scan.name.clone(),
        scan.target_id.clone(),
    );
    if let Some((overview, recs)) = stored_recommendations_from_playbook(scan.playbook_json.as_deref()) {
        input.recommendation_overview = Some(overview);
        input.recommendations = recs;
    } else {
        input.recommendations.clear();
    }

    let report_format = parse_format(format);
    let report_kind = parse_kind(kind);
    let generated = formatter_for(report_format)
        .render(report_kind, &input)
        .await
        .map_err(map_report_err)?;
    Ok((generated, findings.len()))
}

fn downloads_dir(state: &AppState) -> PathBuf {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join("Downloads"))
        .unwrap_or_else(|| state.workspaces_dir().join("downloads"))
}

fn timestamped_filename(filename: &str) -> String {
    let ts = time::OffsetDateTime::now_utc().unix_timestamp();
    if let Some(stem) = filename.strip_suffix(".sarif.json") {
        format!("{stem}-{ts}.sarif.json")
    } else if let Some((stem, ext)) = filename.rsplit_once('.') {
        format!("{stem}-{ts}.{ext}")
    } else {
        format!("{filename}-{ts}")
    }
}

fn unique_path(dir: &Path, filename: &str) -> PathBuf {
    let mut dest = dir.join(filename);
    if !dest.exists() {
        return dest;
    }
    let (stem, ext) = match filename.rsplit_once('.') {
        Some((s, e)) if !filename.ends_with(".sarif.json") => (s.to_string(), format!(".{e}")),
        _ if filename.ends_with(".sarif.json") => (
            filename.trim_end_matches(".sarif.json").to_string(),
            ".sarif.json".to_string(),
        ),
        _ => (filename.to_string(), String::new()),
    };
    for i in 2..1000 {
        dest = dir.join(format!("{stem}-{i}{ext}"));
        if !dest.exists() {
            return dest;
        }
    }
    dir.join(format!(
        "{stem}-{}{ext}",
        time::OffsetDateTime::now_utc().unix_timestamp_nanos()
    ))
}

// ---------------------------------------------------------------------------
// Targets
// ---------------------------------------------------------------------------

#[instrument(skip(state, descriptor))]
pub async fn target_create_op(
    state: &AppState,
    project_id: String,
    name: String,
    target_type: String,
    descriptor: Option<serde_json::Value>,
) -> CommandResult<TargetDto> {
    let descriptor_json = if let Some(descriptor) = descriptor {
        let raw = serde_json::to_string(&descriptor).map_err(|err| {
            CommandError::from(PromptLabError::invalid_input(err.to_string()))
        })?;
        let secrets = SecretStore::new().map_err(CommandError::from)?;
        let (sanitized, _) = sanitize_target_descriptor(&raw, &secrets).map_err(CommandError::from)?;
        Some(
            serde_json::from_str(&sanitized).map_err(|err| {
                CommandError::from(PromptLabError::invalid_input(err.to_string()))
            })?,
        )
    } else {
        None
    };

    let target = state
        .repositories()
        .targets()
        .create(CreateTarget {
            project_id,
            name,
            target_type,
            descriptor_json,
            profile_json: None,
        })
        .await
        .map_err(CommandError::from)?;
    info!(id = %target.id, "target created");
    Ok(target.into())
}

pub async fn target_list_op(state: &AppState, project_id: String) -> CommandResult<Vec<TargetDto>> {
    let targets = state
        .repositories()
        .targets()
        .list_by_project(&project_id)
        .await
        .map_err(CommandError::from)?;
    Ok(targets.into_iter().map(TargetDto::from).collect())
}

pub async fn target_get_op(state: &AppState, target_id: String) -> CommandResult<TargetDto> {
    let target = state
        .repositories()
        .targets()
        .get(&target_id)
        .await
        .map_err(CommandError::from)?;
    Ok(TargetDto::from(target))
}

/// Return a target descriptor with auth secrets resolved for the scan wizard edit form.
pub async fn target_wizard_descriptor_op(
    state: &AppState,
    target_id: String,
) -> CommandResult<TargetDto> {
    let target = state
        .repositories()
        .targets()
        .get(&target_id)
        .await
        .map_err(CommandError::from)?;
    let descriptor_json = target.descriptor_json.clone();
    let mut dto = TargetDto::from(target);
    let secrets = SecretStore::new().map_err(CommandError::from)?;
    let resolved = resolve_descriptor_for_wizard(&descriptor_json, &secrets)
        .map_err(CommandError::from)?;
    dto.descriptor = serde_json::from_str(&resolved).map_err(|err| {
        CommandError::from(PromptLabError::internal(format!(
            "invalid resolved descriptor json: {err}"
        )))
    })?;
    Ok(dto)
}

pub async fn target_update_descriptor_op(
    state: &AppState,
    target_id: String,
    descriptor: serde_json::Value,
) -> CommandResult<TargetDto> {
    let raw = serde_json::to_string(&descriptor).map_err(|err| {
        CommandError::from(PromptLabError::invalid_input(err.to_string()))
    })?;
    let secrets = SecretStore::new().map_err(CommandError::from)?;
    let (sanitized, _) = sanitize_target_descriptor(&raw, &secrets).map_err(CommandError::from)?;
    let target = state
        .repositories()
        .targets()
        .update_descriptor(&target_id, &sanitized)
        .await
        .map_err(CommandError::from)?;
    Ok(TargetDto::from(target))
}

#[instrument(skip(state), fields(id = %id))]
pub async fn target_delete_op(state: &AppState, id: String) -> CommandResult<()> {
    let repos = state.repositories();
    let target = repos.targets().get(&id).await.map_err(CommandError::from)?;

    let scans = repos
        .scans()
        .list_by_project(&target.project_id)
        .await
        .map_err(CommandError::from)?;
    let target_scans: Vec<_> = scans
        .into_iter()
        .filter(|scan| scan.target_id.as_deref() == Some(id.as_str()))
        .collect();

    for scan in &target_scans {
        let _ = state.jobs().request_cancel(&scan.id);
    }

    // Existing DBs used ON DELETE SET NULL for scans.target_id — explicitly remove
    // attack scans (and cascaded findings/results) so they do not linger orphaned.
    for scan in &target_scans {
        repos
            .scans()
            .delete(&scan.id)
            .await
            .map_err(CommandError::from)?;
    }

    repos.targets().delete(&id).await.map_err(CommandError::from)?;
    info!(%id, deleted_scans = target_scans.len(), "target deleted");
    Ok(())
}

// ---------------------------------------------------------------------------
// Scans
// ---------------------------------------------------------------------------

#[instrument(skip(state))]
pub async fn scan_create_op(
    state: &AppState,
    project_id: String,
    target_id: Option<String>,
    name: String,
    status: Option<String>,
) -> CommandResult<ScanDto> {
    let scan = state
        .repositories()
        .scans()
        .create(CreateScan {
            project_id,
            target_id,
            name,
            status,
            playbook_json: None,
        })
        .await
        .map_err(CommandError::from)?;
    info!(id = %scan.id, "scan created");
    Ok(scan.into())
}

pub async fn scan_list_op(state: &AppState, project_id: String) -> CommandResult<Vec<ScanDto>> {
    let scans = state
        .repositories()
        .scans()
        .list_by_project(&project_id)
        .await
        .map_err(CommandError::from)?;
    Ok(scans.into_iter().map(ScanDto::from).collect())
}

pub async fn scan_get_op(state: &AppState, scan_id: String) -> CommandResult<ScanDetailDto> {
    let scan = state
        .repositories()
        .scans()
        .get(&scan_id)
        .await
        .map_err(CommandError::from)?;
    Ok(ScanDetailDto::from_scan(scan))
}

#[instrument(skip(state), fields(id = %id))]
pub async fn scan_delete_op(state: &AppState, id: String) -> CommandResult<()> {
    let _scan = state
        .repositories()
        .scans()
        .get(&id)
        .await
        .map_err(CommandError::from)?;

    let _ = state.jobs().request_cancel(&id);

    state
        .repositories()
        .scans()
        .delete(&id)
        .await
        .map_err(CommandError::from)?;

    info!(%id, "scan deleted");
    Ok(())
}

// ---------------------------------------------------------------------------
// Findings
// ---------------------------------------------------------------------------

pub async fn finding_list_op(state: &AppState, scan_id: String) -> CommandResult<Vec<FindingDto>> {
    let findings = state
        .repositories()
        .findings()
        .list_by_scan(&scan_id)
        .await
        .map_err(CommandError::from)?;
    Ok(findings.into_iter().map(FindingDto::from).collect())
}

pub async fn finding_list_all_op(state: &AppState) -> CommandResult<Vec<FindingDto>> {
    let repos = state.repositories();
    let projects = repos.projects().list().await.map_err(CommandError::from)?;
    let mut findings = Vec::new();

    for project in projects {
        let mut rows = repos
            .findings()
            .list_by_project(&project.id)
            .await
            .map_err(CommandError::from)?;
        findings.append(&mut rows);
    }

    findings.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(findings.into_iter().map(FindingDto::from).collect())
}

#[instrument(skip(state, path))]
pub async fn finding_import_sarif_op(
    state: &AppState,
    project_id: Option<String>,
    path: String,
) -> CommandResult<FindingImportDto> {
    let repos = state.repositories();

    let raw = std::fs::read_to_string(&path).map_err(|err| {
        CommandError::from(PromptLabError::invalid_input(format!(
            "failed to read SARIF file: {err}"
        )))
    })?;

    let bundle = parse_sarif_import(&raw).map_err(map_report_err)?;
    let ctx = &bundle.context;

    let resolved_project_id = resolve_import_project_id(&repos, ctx, project_id).await?;
    let _project = repos
        .projects()
        .get(&resolved_project_id)
        .await
        .map_err(CommandError::from)?;

    let file_stem = std::path::Path::new(&path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("sarif");

    let scan_name = ctx
        .scan_name
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("SARIF import — {file_stem}"));

    let target_id = match ctx.target_id.as_deref() {
        Some(tid) => match repos.targets().get(tid).await {
            Ok(target) if target.project_id == resolved_project_id => Some(target.id),
            _ => None,
        },
        None => None,
    };

    let scan = if let Some(existing_id) = ctx.scan_id.as_deref().filter(|s| !s.trim().is_empty()) {
        match repos.scans().get(existing_id).await {
            Ok(scan) if scan.project_id == resolved_project_id => scan,
            Ok(_) => {
                return Err(CommandError::from(PromptLabError::invalid_input(
                    "SARIF scan_id belongs to a different project",
                )));
            }
            Err(_) => {
                repos
                    .scans()
                    .create(CreateScan {
                        project_id: resolved_project_id.clone(),
                        target_id: target_id.clone(),
                        name: scan_name.clone(),
                        status: Some("completed".into()),
                        playbook_json: Some(serde_json::json!({
                            "source": "sarif_import",
                            "path": path,
                            "original_scan_id": existing_id,
                        })),
                    })
                    .await
                    .map_err(CommandError::from)?
            }
        }
    } else {
        repos
            .scans()
            .create(CreateScan {
                project_id: resolved_project_id.clone(),
                target_id: target_id.clone(),
                name: scan_name,
                status: Some("completed".into()),
                playbook_json: Some(serde_json::json!({
                    "source": "sarif_import",
                    "path": path,
                })),
            })
            .await
            .map_err(CommandError::from)?
    };

    let mut created = Vec::with_capacity(bundle.findings.len());
    for item in bundle.findings {
        let finding = repos
            .findings()
            .create(CreateFinding {
                scan_id: scan.id.clone(),
                project_id: resolved_project_id.clone(),
                target_id: scan.target_id.clone(),
                title: item.title,
                severity: item.severity.as_str().to_string(),
                category: Some(item.category),
                description: Some(item.description),
                evidence_json: Some(item.evidence_json),
                status: Some(item.status),
            })
            .await
            .map_err(CommandError::from)?;
        created.push(FindingDto::from(finding));
    }

    info!(
        project_id = %resolved_project_id,
        scan_id = %scan.id,
        imported = created.len(),
        "SARIF findings imported"
    );

    Ok(FindingImportDto {
        scan_id: scan.id,
        imported_count: created.len() as u32,
        findings: created,
    })
}

fn normalize_finding_status(status: &str) -> CommandResult<String> {
    let normalized = status.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "open" | "confirmed" | "false_positive" | "fixed" => Ok(normalized),
        _ => Err(CommandError::from(PromptLabError::invalid_input(
            "status must be one of: open, confirmed, false_positive, fixed",
        ))),
    }
}

#[instrument(skip(state), fields(id = %id, status = %status))]
pub async fn finding_update_op(
    state: &AppState,
    id: String,
    status: String,
) -> CommandResult<FindingDto> {
    let status = normalize_finding_status(&status)?;
    let finding = state
        .repositories()
        .findings()
        .update(
            &id,
            UpdateFinding {
                status: Some(status),
                ..Default::default()
            },
        )
        .await
        .map_err(CommandError::from)?;

    info!(%id, status = %finding.status, "finding status updated");
    Ok(FindingDto::from(finding))
}

fn evidence_string(obj: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = obj.get(*key).and_then(|v| v.as_str()).map(str::trim) {
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn judge_request_from_finding(
    finding: &promptlab_storage::Finding,
    evidence: &serde_json::Value,
) -> CommandResult<promptlab_judge::JudgeRequest> {
    let obj = evidence.as_object().ok_or_else(|| {
        CommandError::from(PromptLabError::invalid_input(
            "Finding has no structured evidence to re-judge",
        ))
    })?;

    let request = obj.get("request").and_then(|v| v.as_object());
    let response = obj.get("response").and_then(|v| v.as_object());

    let payload = evidence_string(obj, &["payload"])
        .or_else(|| request.and_then(|r| evidence_string(r, &["body"])))
        .ok_or_else(|| {
            CommandError::from(PromptLabError::invalid_input(
                "Finding evidence is missing a payload to re-judge",
            ))
        })?;

    let probe_id = evidence_string(obj, &["payload_id", "payloadId"])
        .unwrap_or_else(|| finding.id.clone());

    let response_text = response
        .and_then(|r| r.get("normalized"))
        .and_then(|n| {
            serde_json::from_value::<promptlab_harness::NormalizedResponse>(n.clone())
                .ok()
                .map(|normalized| normalized.judge_text())
        })
        .or_else(|| response.and_then(|r| evidence_string(r, &["body"])))
        .or_else(|| evidence_string(obj, &["response_excerpt", "responseExcerpt"]))
        .ok_or_else(|| {
            CommandError::from(PromptLabError::invalid_input(
                "Finding evidence is missing a response to re-judge",
            ))
        })?;

    let attack_category = finding
        .category
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "general".into());

    let mut context = serde_json::json!({
        "finding_id": finding.id,
        "scan_id": finding.scan_id,
        "project_id": finding.project_id,
    });
    if let Some(response) = response {
        if let Some(status) = response.get("status") {
            context["status_code"] = status.clone();
        }
        if let Some(body) = response.get("body") {
            context["raw_response"] = body.clone();
        }
    }

    Ok(promptlab_judge::JudgeRequest {
        probe_id,
        attack_category,
        payload,
        response_text,
        context,
    })
}

fn merge_rejudge_evidence(
    existing: Option<&str>,
    verdict: &promptlab_judge::JudgeVerdict,
) -> CommandResult<serde_json::Value> {
    let mut judge_json = serde_json::to_value(verdict).map_err(|err| {
        CommandError::from(PromptLabError::internal(format!(
            "Failed to serialize judge verdict: {err}"
        )))
    })?;

    if let Some(obj) = judge_json.as_object_mut() {
        let judged_at = verdict
            .judged_at
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| verdict.judged_at.to_string());
        obj.insert("judged_at".into(), serde_json::json!(judged_at));
    }

    let mut evidence = existing
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    let obj = evidence.as_object_mut().ok_or_else(|| {
        CommandError::from(PromptLabError::invalid_input(
            "Finding evidence_json must be a JSON object",
        ))
    })?;

    obj.insert("judge".into(), judge_json);
    obj.insert(
        "confidence".into(),
        serde_json::json!(verdict.confidence),
    );
    obj.insert("verdict".into(), serde_json::json!(verdict.verdict));
    obj.insert("explanation".into(), serde_json::json!(verdict.summary));
    obj.insert(
        "indicators".into(),
        serde_json::json!(verdict.evidence.clone()),
    );

    Ok(evidence)
}

fn judge_severity_label(severity: promptlab_judge::Severity) -> &'static str {
    match severity {
        promptlab_judge::Severity::Info => "info",
        promptlab_judge::Severity::Low => "low",
        promptlab_judge::Severity::Medium => "medium",
        promptlab_judge::Severity::High => "high",
        promptlab_judge::Severity::Critical => "critical",
    }
}

#[instrument(skip(state), fields(id = %id))]
pub async fn finding_rejudge_op(state: &AppState, id: String) -> CommandResult<FindingDto> {
    let repos = state.repositories();
    let finding = repos
        .findings()
        .get(&id)
        .await
        .map_err(CommandError::from)?;

    let evidence_value = finding
        .evidence_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    let judge_request = judge_request_from_finding(&finding, &evidence_value)?;

    let judge = Arc::new(
        crate::commands::attack::build_judge_for_category(
            state.data_dir(),
            Arc::clone(state.inference_manager()),
            Arc::clone(state.model_manager()),
            state.model_provider().clone(),
            Arc::clone(state.runtime_manager()),
            &repos,
        )
        .await?,
    );
    let orchestrator = crate::commands::attack::judge_coordinator_llm(
        state.data_dir(),
        Arc::clone(state.inference_manager()),
        Arc::clone(state.model_manager()),
        state.model_provider().clone(),
        Arc::clone(state.runtime_manager()),
    );

    let outcome = promptlab_agent::JudgeCoordinatorAgent::run_with_orchestrator(
        &judge_request,
        judge,
        orchestrator,
    )
        .await
        .map_err(|err| {
            CommandError::from(PromptLabError::internal(format!(
                "Re-judge failed: {err}"
            )))
        })?;

    let verdict = outcome.verdict;
    let evidence_json = merge_rejudge_evidence(finding.evidence_json.as_deref(), &verdict)?;
    let severity = verdict
        .severity
        .map(judge_severity_label)
        .map(str::to_string);

    let updated = repos
        .findings()
        .update(
            &id,
            UpdateFinding {
                description: Some(verdict.summary.clone()),
                severity,
                evidence_json: Some(evidence_json),
                ..Default::default()
            },
        )
        .await
        .map_err(CommandError::from)?;

    info!(
        %id,
        verdict = %verdict.verdict,
        confidence = verdict.confidence,
        "finding re-judged"
    );
    Ok(FindingDto::from(updated))
}

#[instrument(skip(state), fields(id = %id))]
pub async fn finding_delete_op(state: &AppState, id: String) -> CommandResult<()> {
    state
        .repositories()
        .findings()
        .delete(&id)
        .await
        .map_err(CommandError::from)?;

    info!(%id, "finding deleted");
    Ok(())
}

async fn resolve_import_project_id(
    repos: &promptlab_storage::Repositories,
    ctx: &promptlab_report::SarifRunContext,
    override_project_id: Option<String>,
) -> CommandResult<String> {
    if let Some(id) = override_project_id.filter(|s| !s.trim().is_empty()) {
        repos.projects().get(&id).await.map_err(CommandError::from)?;
        return Ok(id);
    }

    if let Some(id) = ctx.project_id.as_deref().filter(|s| !s.trim().is_empty()) {
        if repos.projects().get(id).await.is_ok() {
            return Ok(id.to_string());
        }
    }

    if let Some(name) = ctx.project_name.as_deref().filter(|s| !s.trim().is_empty()) {
        let projects = repos.projects().list().await.map_err(CommandError::from)?;
        if let Some(project) = projects.iter().find(|p| p.name.eq_ignore_ascii_case(name)) {
            return Ok(project.id.clone());
        }
    }

    Err(CommandError::from(PromptLabError::invalid_input(
        "SARIF is missing a resolvable project_id/project_name; select a destination project",
    )))
}

// ---------------------------------------------------------------------------
// Reports
// ---------------------------------------------------------------------------

#[instrument(skip(state))]
pub async fn report_generate_op(
    state: &AppState,
    project_id: String,
    scan_id: String,
    format: Option<String>,
    kind: Option<String>,
) -> CommandResult<ReportDto> {
    let (generated, findings_len) = render_scan_report(
        state,
        &project_id,
        &scan_id,
        format.as_deref(),
        kind.as_deref(),
    )
    .await?;

    let engine = ReportingEngine::new(state.reports_dir()).map_err(map_report_err)?;
    let file_path = engine.output_dir().join(&generated.filename);
    std::fs::write(&file_path, &generated.bytes)
        .map_err(|err| CommandError::from(PromptLabError::from(err)))?;

    info!(scan_id = %scan_id, file = %file_path.display(), findings = findings_len, "report generated");

    let record = state
        .repositories()
        .reports()
        .create(CreateReport {
            project_id: project_id.clone(),
            scan_id: Some(scan_id.clone()),
            name: "PromptLab - Security Scan Report".into(),
            format: generated.format.as_str().to_string(),
            status: Some("completed".into()),
            file_path: Some(file_path.to_string_lossy().into_owned()),
            metadata_json: Some(serde_json::json!({
                "findings": findings_len,
                "kind": generated.kind.as_str(),
                "exported": false,
            })),
        })
        .await
        .map_err(CommandError::from)?;

    Ok(record.into())
}

pub async fn report_read_op(state: &AppState, id: String) -> CommandResult<ReportContentDto> {
    let report = state.repositories().reports().get(&id).await.map_err(CommandError::from)?;
    let path = report
        .file_path
        .clone()
        .ok_or_else(|| CommandError::invalid_input("Report has no saved file"))?;
    let content = std::fs::read_to_string(&path)
        .map_err(|err| CommandError::from(PromptLabError::from(err)))?;
    Ok(ReportContentDto {
        id: report.id,
        name: report.name,
        format: report.format,
        content,
    })
}

pub async fn report_export_op(state: &AppState, id: String) -> CommandResult<String> {
    let report = state.repositories().reports().get(&id).await.map_err(CommandError::from)?;
    let src = report
        .file_path
        .clone()
        .ok_or_else(|| CommandError::invalid_input("Report has no saved file"))?;

    let downloads = downloads_dir(state);
    std::fs::create_dir_all(&downloads).map_err(|err| CommandError::from(PromptLabError::from(err)))?;

    let file_name = std::path::Path::new(&src)
        .file_name()
        .and_then(|f| f.to_str())
        .map(timestamped_filename)
        .unwrap_or_else(|| timestamped_filename(&format!("{}.{}", report.name, report.format)));
    let dest = unique_path(&downloads, &file_name);

    std::fs::copy(&src, &dest).map_err(|err| CommandError::from(PromptLabError::from(err)))?;
    let dest_str = dest.to_string_lossy().into_owned();
    info!(report_id = %report.id, dest = %dest_str, "report exported");
    Ok(dest_str)
}

#[instrument(skip(state))]
pub async fn report_export_scan_op(
    state: &AppState,
    project_id: String,
    scan_id: String,
    format: Option<String>,
    kind: Option<String>,
) -> CommandResult<String> {
    let (generated, findings_len) = render_scan_report(
        state,
        &project_id,
        &scan_id,
        format.as_deref(),
        kind.as_deref(),
    )
    .await?;

    // Persist to report history (Recent Activity + Reports list), then copy to Downloads.
    let engine = ReportingEngine::new(state.reports_dir()).map_err(map_report_err)?;
    let history_path = engine.output_dir().join(&generated.filename);
    std::fs::write(&history_path, &generated.bytes)
        .map_err(|err| CommandError::from(PromptLabError::from(err)))?;

    state
        .repositories()
        .reports()
        .create(CreateReport {
            project_id: project_id.clone(),
            scan_id: Some(scan_id.clone()),
            name: "PromptLab - Security Scan Report".into(),
            format: generated.format.as_str().to_string(),
            status: Some("completed".into()),
            file_path: Some(history_path.to_string_lossy().into_owned()),
            metadata_json: Some(serde_json::json!({
                "findings": findings_len,
                "kind": generated.kind.as_str(),
                "exported": true,
            })),
        })
        .await
        .map_err(CommandError::from)?;

    let downloads = downloads_dir(state);
    std::fs::create_dir_all(&downloads).map_err(|err| CommandError::from(PromptLabError::from(err)))?;
    let dest = unique_path(&downloads, &timestamped_filename(&generated.filename));
    std::fs::write(&dest, &generated.bytes)
        .map_err(|err| CommandError::from(PromptLabError::from(err)))?;

    let dest_str = dest.to_string_lossy().into_owned();
    info!(
        scan_id = %scan_id,
        dest = %dest_str,
        findings = findings_len,
        "report exported"
    );
    Ok(dest_str)
}

pub async fn report_list_op(state: &AppState, project_id: String) -> CommandResult<Vec<ReportDto>> {
    let reports = state
        .repositories()
        .reports()
        .list_by_project(&project_id)
        .await
        .map_err(CommandError::from)?;
    Ok(reports.into_iter().map(ReportDto::from).collect())
}

pub async fn report_list_all_op(state: &AppState) -> CommandResult<Vec<ReportDto>> {
    let repos = state.repositories();
    let projects = repos.projects().list().await.map_err(CommandError::from)?;
    let mut reports = Vec::new();

    for project in projects {
        let mut rows = repos
            .reports()
            .list_by_project(&project.id)
            .await
            .map_err(CommandError::from)?;
        reports.append(&mut rows);
    }

    reports.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(reports.into_iter().map(ReportDto::from).collect())
}

// ===========================================================================
// Tauri command wrappers (thin; delegate to the *_op functions above)
// ===========================================================================

#[tauri::command]
pub async fn target_create(
    state: State<'_, AppState>,
    project_id: String,
    name: String,
    target_type: String,
    descriptor: Option<serde_json::Value>,
) -> CommandResult<TargetDto> {
    target_create_op(state.inner(), project_id, name, target_type, descriptor).await
}

#[tauri::command]
pub async fn target_list(
    state: State<'_, AppState>,
    project_id: String,
) -> CommandResult<Vec<TargetDto>> {
    target_list_op(state.inner(), project_id).await
}

#[tauri::command]
pub async fn target_get(state: State<'_, AppState>, id: String) -> CommandResult<TargetDto> {
    target_get_op(state.inner(), id).await
}

#[tauri::command]
pub async fn target_wizard_descriptor(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<TargetDto> {
    target_wizard_descriptor_op(state.inner(), id).await
}

#[tauri::command]
pub async fn target_update_descriptor(
    state: State<'_, AppState>,
    id: String,
    descriptor: serde_json::Value,
) -> CommandResult<TargetDto> {
    target_update_descriptor_op(state.inner(), id, descriptor).await
}

#[tauri::command]
pub async fn target_delete(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    target_delete_op(state.inner(), id).await
}

#[tauri::command]
pub async fn scan_create(
    state: State<'_, AppState>,
    project_id: String,
    target_id: Option<String>,
    name: String,
    status: Option<String>,
) -> CommandResult<ScanDto> {
    scan_create_op(state.inner(), project_id, target_id, name, status).await
}

#[tauri::command]
pub async fn scan_list(
    state: State<'_, AppState>,
    project_id: String,
) -> CommandResult<Vec<ScanDto>> {
    scan_list_op(state.inner(), project_id).await
}

#[tauri::command]
pub async fn scan_get(state: State<'_, AppState>, id: String) -> CommandResult<ScanDetailDto> {
    scan_get_op(state.inner(), id).await
}

#[tauri::command]
pub async fn scan_delete(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    scan_delete_op(state.inner(), id).await
}

#[tauri::command]
pub async fn finding_list(
    state: State<'_, AppState>,
    scan_id: String,
) -> CommandResult<Vec<FindingDto>> {
    finding_list_op(state.inner(), scan_id).await
}

#[tauri::command]
pub async fn finding_list_all(state: State<'_, AppState>) -> CommandResult<Vec<FindingDto>> {
    finding_list_all_op(state.inner()).await
}

#[tauri::command]
pub async fn finding_import_sarif(
    state: State<'_, AppState>,
    path: String,
    project_id: Option<String>,
) -> CommandResult<FindingImportDto> {
    finding_import_sarif_op(state.inner(), project_id, path).await
}

#[tauri::command]
pub async fn finding_update(
    state: State<'_, AppState>,
    id: String,
    status: String,
) -> CommandResult<FindingDto> {
    finding_update_op(state.inner(), id, status).await
}

#[tauri::command]
pub async fn finding_rejudge(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<FindingDto> {
    finding_rejudge_op(state.inner(), id).await
}

#[tauri::command]
pub async fn finding_delete(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    finding_delete_op(state.inner(), id).await
}

#[tauri::command]
pub async fn report_generate(
    state: State<'_, AppState>,
    project_id: String,
    scan_id: String,
    format: Option<String>,
    kind: Option<String>,
) -> CommandResult<ReportDto> {
    report_generate_op(state.inner(), project_id, scan_id, format, kind).await
}

#[tauri::command]
pub async fn report_list(
    state: State<'_, AppState>,
    project_id: String,
) -> CommandResult<Vec<ReportDto>> {
    report_list_op(state.inner(), project_id).await
}

#[tauri::command]
pub async fn report_list_all(state: State<'_, AppState>) -> CommandResult<Vec<ReportDto>> {
    report_list_all_op(state.inner()).await
}

#[tauri::command]
pub async fn report_read(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<ReportContentDto> {
    report_read_op(state.inner(), id).await
}

#[tauri::command]
pub async fn report_export(state: State<'_, AppState>, id: String) -> CommandResult<String> {
    report_export_op(state.inner(), id).await
}

#[tauri::command]
pub async fn report_export_scan(
    state: State<'_, AppState>,
    project_id: String,
    scan_id: String,
    format: Option<String>,
    kind: Option<String>,
) -> CommandResult<String> {
    report_export_scan_op(state.inner(), project_id, scan_id, format, kind).await
}
