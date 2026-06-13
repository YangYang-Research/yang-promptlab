//! Domain IPC commands operating directly on SQLite via the repository layer.
//!
//! Each command is a thin `#[tauri::command]` wrapper over a testable `*_op`
//! function that takes `&AppState`, so the same logic is exercised by the
//! integration tests without a Tauri runtime.

use aisec_auth::{sanitize_target_descriptor, SecretStore};
use aisec_core::AisecError;
use aisec_report::{
    ReportDataBuilder, ReportFormat, ReportKind, ReportingEngine, StorageFindingRow,
};
use aisec_storage::{
    CreateReport, CreateScan, CreateTarget, FindingRepository, ProjectRepository, ReportRepository,
    ScanRepository, TargetRepository,
};
use tauri::State;
use tracing::{info, instrument};

use crate::dto::{FindingDto, ReportContentDto, ReportDto, ScanDetailDto, ScanDto, TargetDto};
use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

fn map_report_err(err: aisec_report::ReportError) -> CommandError {
    CommandError::from(AisecError::internal(err.to_string()))
}

fn parse_format(value: Option<&str>) -> ReportFormat {
    match value.map(str::to_ascii_lowercase).as_deref() {
        Some("pdf") => ReportFormat::Pdf,
        Some("json") => ReportFormat::Json,
        Some("sarif") => ReportFormat::Sarif,
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
            CommandError::from(AisecError::invalid_input(err.to_string()))
        })?;
        let secrets = SecretStore::new().map_err(CommandError::from)?;
        let (sanitized, _) = sanitize_target_descriptor(&raw, &secrets).map_err(CommandError::from)?;
        Some(
            serde_json::from_str(&sanitized).map_err(|err| {
                CommandError::from(AisecError::invalid_input(err.to_string()))
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
    let repos = state.repositories();

    let project = repos.projects().get(&project_id).await.map_err(CommandError::from)?;
    let scan = repos.scans().get(&scan_id).await.map_err(CommandError::from)?;
    let findings = repos
        .findings()
        .list_by_scan(&scan_id)
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
    let input = ReportDataBuilder::build(
        scan_id.clone(),
        project.name.clone(),
        target_name,
        report_findings,
    );

    let report_format = parse_format(format.as_deref());
    let report_kind = parse_kind(kind.as_deref());

    let engine = ReportingEngine::new(state.reports_dir()).map_err(map_report_err)?;
    let generated = engine
        .generate(report_kind, report_format, &input)
        .await
        .map_err(map_report_err)?;
    let file_path = state.reports_dir().join(&generated.filename);

    info!(scan_id = %scan_id, file = %file_path.display(), findings = findings.len(), "report generated");

    let record = repos
        .reports()
        .create(CreateReport {
            project_id: project_id.clone(),
            scan_id: Some(scan_id.clone()),
            name: format!("{} {} report", report_kind.as_str(), report_format.as_str()),
            format: report_format.as_str().to_string(),
            status: Some("completed".into()),
            file_path: Some(file_path.to_string_lossy().into_owned()),
            metadata_json: Some(serde_json::json!({
                "findings": findings.len(),
                "kind": report_kind.as_str(),
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
        .map_err(|err| CommandError::from(AisecError::from(err)))?;
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

    let downloads = std::env::var_os("HOME")
        .map(|home| std::path::PathBuf::from(home).join("Downloads"))
        .unwrap_or_else(|| state.data_dir().join("downloads"));
    std::fs::create_dir_all(&downloads).map_err(|err| CommandError::from(AisecError::from(err)))?;

    let file_name = std::path::Path::new(&src)
        .file_name()
        .map(|f| f.to_os_string())
        .unwrap_or_else(|| format!("{}.{}", report.name, report.format).into());
    let dest = downloads.join(file_name);

    std::fs::copy(&src, &dest).map_err(|err| CommandError::from(AisecError::from(err)))?;
    let dest_str = dest.to_string_lossy().into_owned();
    info!(report_id = %report.id, dest = %dest_str, "report exported");
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
