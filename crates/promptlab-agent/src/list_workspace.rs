//! Host-backed workspace tools for Yazg (scoped reads — no full DB dump).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Compact finding row for listings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFindingSummary {
    pub id: String,
    pub project_id: String,
    pub scan_id: String,
    pub title: String,
    pub severity: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

/// Scan row for listings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceScanSummary {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
}

/// Target row for listings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTargetSummary {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub target_type: String,
}

/// Project row with counts only (used by list_workspace).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceProjectSummary {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub findings_count: usize,
    #[serde(default)]
    pub targets_count: usize,
    #[serde(default)]
    pub scans_count: usize,
}

/// Totals for a project list snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTotals {
    pub projects: usize,
    pub targets: usize,
    pub scans: usize,
    pub findings: usize,
    #[serde(default)]
    pub findings_truncated: usize,
}

/// Slim workspace listing: projects + aggregate counts only.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInventory {
    pub projects: Vec<WorkspaceProjectSummary>,
    pub totals: WorkspaceTotals,
}

impl WorkspaceInventory {
    pub fn to_observation(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "list_workspace OK — {} project(s). Use project_detail / list_scan / list_findings for more.",
            self.projects.len()
        ));
        lines.push(format!(
            "Totals: projects={} targets={} scans={} findings={}",
            self.totals.projects, self.totals.targets, self.totals.scans, self.totals.findings
        ));
        if self.projects.is_empty() {
            lines.push("Projects: (none)".into());
        } else {
            lines.push("Projects:".into());
            for p in &self.projects {
                let desc = p
                    .description
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("-");
                lines.push(format!(
                    "  - id={} name={} targets={} scans={} findings={} description={}",
                    p.id, p.name, p.targets_count, p.scans_count, p.findings_count, desc
                ));
            }
        }
        lines.join("\n")
    }

    pub fn compact_user_reply_for_goal(&self, goal: &str) -> String {
        if let Some(project) = self.project_named_in_goal(goal) {
            return format!(
                "**{}**\n\n- Id: `{}`\n- Targets: **{}**\n- Scans: **{}**\n- Findings: **{}**\n\nUse `project_detail` / `list_scan` / `list_findings` for more.",
                project.name,
                project.id,
                project.targets_count,
                project.scans_count,
                project.findings_count
            );
        }
        format!(
            "Workspace: **{}** projects, **{}** targets, **{}** scans, **{}** findings.\n\nProjects:\n{}",
            self.totals.projects,
            self.totals.targets,
            self.totals.scans,
            self.totals.findings,
            if self.projects.is_empty() {
                "- (none)".into()
            } else {
                self.projects
                    .iter()
                    .map(|p| {
                        format!(
                            "- **{}** (`{}`) — {} findings",
                            p.name, p.id, p.findings_count
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        )
    }

    /// Exact id or case-insensitive name match.
    pub fn find_project(&self, key: &str) -> Option<&WorkspaceProjectSummary> {
        let key = key.trim();
        if key.is_empty() {
            return None;
        }
        if let Some(p) = self.projects.iter().find(|p| p.id == key) {
            return Some(p);
        }
        let lower = key.to_lowercase();
        self.projects
            .iter()
            .find(|p| p.name.to_lowercase() == lower)
    }

    pub fn missing_project_reply(&self, requested: &str) -> String {
        let requested = requested.trim();
        let candidates = if self.projects.is_empty() {
            "- (none)".into()
        } else {
            self.projects
                .iter()
                .map(|p| format!("- **{}** (`{}`)", p.name, p.id))
                .collect::<Vec<_>>()
                .join("\n")
        };
        format!(
            "Không có project **{requested}** trong workspace.\n\nProjects hiện có:\n{candidates}"
        )
    }

    pub(crate) fn project_named_in_goal(&self, goal: &str) -> Option<&WorkspaceProjectSummary> {
        if goal.trim().is_empty() || self.projects.is_empty() {
            return None;
        }
        if let Some(asked) = extract_requested_project_name(goal) {
            return self.find_project(&asked);
        }
        // Fallback: project name appears as a whole token in the goal (not a substring).
        let tokens: Vec<String> = goal
            .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
            .filter(|t| !t.is_empty())
            .map(|t| t.to_lowercase())
            .collect();
        let mut ranked: Vec<&WorkspaceProjectSummary> = self
            .projects
            .iter()
            .filter(|p| {
                let name = p.name.trim().to_lowercase();
                !name.is_empty() && tokens.iter().any(|t| t == &name)
            })
            .collect();
        ranked.sort_by(|a, b| b.name.len().cmp(&a.name.len()));
        ranked.into_iter().next()
    }
}

/// Extract a requested project name from goals like "project WebApp" / "thong tin project X".
/// Returns None for inventory-style asks ("project nào", "projects in workspace").
pub fn extract_requested_project_name(goal: &str) -> Option<String> {
    let lower = goal.to_lowercase();
    let idx = lower.find("project ")?;
    let rest = goal[idx + "project ".len()..].trim();
    let token = rest
        .split_whitespace()
        .next()?
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_');
    if token.is_empty() {
        return None;
    }
    const STOP: &[&str] = &[
        "có", "nào", "nao", "trong", "workspace", "workspcae", "the", "a", "an", "của", "cua",
        "thông", "thong", "tin", "information", "info", "overview", "list", "các", "cac", "all",
        "những", "nhung", "với", "voi", "and", "or", "of", "in", "for",
    ];
    if STOP.iter().any(|s| token.eq_ignore_ascii_case(s)) {
        return None;
    }
    Some(token.to_string())
}

/// Project detail: metadata + targets + scans (no finding rows).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    pub project: WorkspaceProjectSummary,
    pub targets: Vec<WorkspaceTargetSummary>,
    pub scans: Vec<WorkspaceScanSummary>,
}

impl ProjectDetail {
    pub fn to_observation(&self) -> String {
        let mut lines = Vec::new();
        let p = &self.project;
        lines.push(format!(
            "project_detail OK — {} (`{}`) findings={} targets={} scans={}",
            p.name, p.id, p.findings_count, p.targets_count, p.scans_count
        ));
        if let Some(desc) = p.description.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            lines.push(format!("description: {desc}"));
        }
        lines.push("Targets:".into());
        if self.targets.is_empty() {
            lines.push("  (none)".into());
        } else {
            for t in &self.targets {
                lines.push(format!(
                    "  - id={} name={} type={}",
                    t.id, t.name, t.target_type
                ));
            }
        }
        lines.push("Scans:".into());
        if self.scans.is_empty() {
            lines.push("  (none)".into());
        } else {
            for s in &self.scans {
                lines.push(format!(
                    "  - id={} name={} status={} target={}",
                    s.id,
                    s.name,
                    s.status,
                    s.target_id.as_deref().unwrap_or("-")
                ));
            }
        }
        lines.push(
            "Next: list_findings(project=...) or scan_detail(scan_id=...) or list_scan(project=...)."
                .into(),
        );
        lines.join("\n")
    }

    pub fn compact_user_reply(&self) -> String {
        let p = &self.project;
        let targets = self
            .targets
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let scans = self
            .scans
            .iter()
            .map(|s| format!("{} ({})", s.name, s.status))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "**{}**\n\n- Id: `{}`\n- Findings: **{}**\n- Targets ({}): {}\n- Scans ({}): {}\n\nAsk for a scan or finding (e.g. list_findings / finding #1) for details.",
            p.name,
            p.id,
            p.findings_count,
            self.targets.len(),
            if targets.is_empty() { "(none)" } else { &targets },
            self.scans.len(),
            if scans.is_empty() { "(none)" } else { &scans }
        )
    }
}

/// Scans for one project.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanList {
    pub project_id: String,
    pub project_name: String,
    pub scans: Vec<WorkspaceScanSummary>,
}

impl ScanList {
    pub fn to_observation(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "list_scan OK — project={} (`{}`) scans={}",
            self.project_name,
            self.project_id,
            self.scans.len()
        ));
        if self.scans.is_empty() {
            lines.push("Scans: (none)".into());
        } else {
            for (i, s) in self.scans.iter().enumerate() {
                lines.push(format!(
                    "  {}. id={} name={} status={} target={}",
                    i + 1,
                    s.id,
                    s.name,
                    s.status,
                    s.target_id.as_deref().unwrap_or("-")
                ));
            }
        }
        lines.push("Next: scan_detail(scan_id=...) for findings on a scan.".into());
        lines.join("\n")
    }
}

/// One scan + capped findings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanDetail {
    pub scan: WorkspaceScanSummary,
    pub project_name: String,
    pub findings: Vec<WorkspaceFindingSummary>,
    pub findings_total: usize,
    pub findings_truncated: usize,
}

impl ScanDetail {
    pub fn to_observation(&self) -> String {
        let mut lines = Vec::new();
        let s = &self.scan;
        lines.push(format!(
            "scan_detail OK — {} (`{}`) project={} status={} findings={} (listed {}, truncated {})",
            s.name,
            s.id,
            self.project_name,
            s.status,
            self.findings_total,
            self.findings.len(),
            self.findings_truncated
        ));
        lines.push("Findings (1-indexed for this scan):".into());
        if self.findings.is_empty() {
            lines.push("  (none)".into());
        } else {
            for (i, f) in self.findings.iter().enumerate() {
                lines.push(format!(
                    "  {}. id={} severity={} status={} title={}",
                    i + 1,
                    f.id,
                    f.severity,
                    f.status,
                    f.title
                ));
            }
        }
        lines.push("Next: finding_detail(finding_id=...) for full detail.".into());
        lines.join("\n")
    }
}

/// Paginated findings for a project or scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindingList {
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub scan_id: Option<String>,
    pub findings: Vec<WorkspaceFindingSummary>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

impl FindingList {
    pub fn to_observation(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "list_findings OK — total={} offset={} limit={} listed={} project={} scan={}",
            self.total,
            self.offset,
            self.limit,
            self.findings.len(),
            self.project_name.as_deref().unwrap_or("-"),
            self.scan_id.as_deref().unwrap_or("-")
        ));
        lines.push("Findings (1-indexed within this page; global index = offset + n):".into());
        if self.findings.is_empty() {
            lines.push("  (none)".into());
        } else {
            for (i, f) in self.findings.iter().enumerate() {
                let n = self.offset + i + 1;
                lines.push(format!(
                    "  {}. id={} severity={} status={} title={}",
                    n, f.id, f.severity, f.status, f.title
                ));
            }
        }
        lines.push("Next: finding_detail(finding_id=...) or finding_detail(project=..., index=N).".into());
        lines.join("\n")
    }

    pub fn compact_finding_at_index(&self, one_based: usize) -> Option<String> {
        if one_based == 0 || one_based <= self.offset {
            return None;
        }
        let idx = one_based - self.offset - 1;
        let f = self.findings.get(idx)?;
        Some(format_finding_markdown(one_based, f, self.project_name.as_deref()))
    }
}

/// Full finding row for the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindingDetail {
    pub index: Option<usize>,
    pub finding: WorkspaceFindingSummary,
    pub project_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_json: Option<String>,
}

impl FindingDetail {
    pub fn to_observation(&self) -> String {
        let f = &self.finding;
        let mut lines = Vec::new();
        lines.push(format!(
            "finding_detail OK — id={} project={} severity={} status={}",
            f.id, self.project_name, f.severity, f.status
        ));
        if let Some(i) = self.index {
            lines.push(format!("index: {i}"));
        }
        lines.push(format!("title: {}", f.title));
        if let Some(cat) = f.category.as_deref() {
            lines.push(format!("category: {cat}"));
        }
        if let Some(desc) = self.description.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            lines.push(format!("description: {desc}"));
        }
        if let Some(ev) = self
            .evidence_json
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let clipped = if ev.chars().count() > 1200 {
                let t: String = ev.chars().take(1200).collect();
                format!("{t}…")
            } else {
                ev.to_string()
            };
            lines.push(format!("evidence_json: {clipped}"));
        }
        lines.join("\n")
    }

    pub fn compact_user_reply(&self) -> String {
        let f = &self.finding;
        let idx = self.index.map(|i| format!("#{i} ")).unwrap_or_default();
        let mut out = format!(
            "### Finding {idx}\n\n- **Title:** {}\n- **Severity:** {}\n- **Status:** {}\n- **Project:** {}\n- **Id:** `{}`\n- **Scan:** `{}`",
            f.title, f.severity, f.status, self.project_name, f.id, f.scan_id
        );
        if let Some(desc) = self.description.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            out.push_str(&format!("\n\n{desc}"));
        }
        out
    }
}

/// Targets for one project.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetList {
    pub project_id: String,
    pub project_name: String,
    pub targets: Vec<WorkspaceTargetSummary>,
}

impl TargetList {
    pub fn to_observation(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "list_targets OK — project={} (`{}`) targets={}",
            self.project_name,
            self.project_id,
            self.targets.len()
        ));
        if self.targets.is_empty() {
            lines.push("Targets: (none)".into());
        } else {
            for (i, t) in self.targets.iter().enumerate() {
                lines.push(format!(
                    "  {}. id={} name={} type={}",
                    i + 1,
                    t.id,
                    t.name,
                    t.target_type
                ));
            }
        }
        lines.push("Next: target_detail(target_id=...) for profile summary.".into());
        lines.join("\n")
    }

    pub fn compact_user_reply(&self) -> String {
        let mut out = format!(
            "### Targets in {}\n\n",
            self.project_name
        );
        if self.targets.is_empty() {
            out.push_str("_No targets in this project._");
            return out;
        }
        for (i, t) in self.targets.iter().enumerate() {
            out.push_str(&format!(
                "{}. **{}** (`{}`) — {}\n",
                i + 1,
                t.name,
                t.id,
                t.target_type
            ));
        }
        out
    }
}

/// One target with a clipped profile summary (not full JSON dump).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetDetail {
    pub target: WorkspaceTargetSummary,
    pub project_name: String,
    pub profile_summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptor_summary: Option<String>,
}

impl TargetDetail {
    pub fn to_observation(&self) -> String {
        let t = &self.target;
        format!(
            "target_detail OK — {} (`{}`) project={} type={}\nprofile: {}\ndescriptor: {}",
            t.name,
            t.id,
            self.project_name,
            t.target_type,
            self.profile_summary,
            self.descriptor_summary.as_deref().unwrap_or("-")
        )
    }

    pub fn compact_user_reply(&self) -> String {
        format!(
            "### Target {}\n\n- **Id:** `{}`\n- **Project:** {}\n- **Type:** {}\n- **Profile:** {}\n- **Descriptor:** {}",
            self.target.name,
            self.target.id,
            self.project_name,
            self.target.target_type,
            self.profile_summary,
            self.descriptor_summary.as_deref().unwrap_or("-")
        )
    }
}

/// Reports for a project (or workspace-wide).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportList {
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub reports: Vec<WorkspaceReportSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceReportSummary {
    pub id: String,
    pub project_id: String,
    pub scan_id: Option<String>,
    pub name: String,
    pub format: String,
    pub status: String,
    #[serde(default)]
    pub finding_count: u64,
}

impl ReportList {
    pub fn to_observation(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "list_reports OK — project={} reports={}",
            self.project_name.as_deref().unwrap_or("(all)"),
            self.reports.len()
        ));
        if self.reports.is_empty() {
            lines.push("Reports: (none)".into());
        } else {
            for (i, r) in self.reports.iter().enumerate() {
                lines.push(format!(
                    "  {}. id={} name={} format={} status={} findings={} scan={}",
                    i + 1,
                    r.id,
                    r.name,
                    r.format,
                    r.status,
                    r.finding_count,
                    r.scan_id.as_deref().unwrap_or("-")
                ));
            }
        }
        lines.push("Next: report_detail(report_id=...) to read content.".into());
        lines.join("\n")
    }
}

/// One report with clipped content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportDetail {
    pub report: WorkspaceReportSummary,
    pub project_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// Clipped file content (HTML/markdown/text) for agent context.
    pub content_preview: String,
    pub content_truncated: bool,
}

impl ReportDetail {
    pub fn to_observation(&self) -> String {
        let r = &self.report;
        format!(
            "report_detail OK — {} (`{}`) project={} format={} status={} findings={}\nfile={}\npreview_truncated={}\n---\n{}",
            r.name,
            r.id,
            self.project_name,
            r.format,
            r.status,
            r.finding_count,
            self.file_path.as_deref().unwrap_or("-"),
            self.content_truncated,
            self.content_preview
        )
    }
}

pub const MAX_REPORT_PREVIEW_CHARS: usize = 4000;

fn format_finding_markdown(
    one_based: usize,
    finding: &WorkspaceFindingSummary,
    project_name: Option<&str>,
) -> String {
    format!(
        "### Finding #{one_based}\n\n- **Title:** {}\n- **Severity:** {}\n- **Status:** {}\n- **Project:** {}\n- **Id:** `{}`\n- **Scan:** `{}`",
        finding.title,
        finding.severity,
        finding.status,
        project_name.unwrap_or(finding.project_id.as_str()),
        finding.id,
        finding.scan_id
    )
}

/// Default page size for finding lists / scan_detail findings.
pub const DEFAULT_FINDINGS_LIMIT: usize = 20;
pub const MAX_FINDINGS_LIMIT: usize = 50;

/// Host implements scoped SQLite reads for Yazg workspace tools.
#[async_trait]
pub trait WorkspaceTools: Send + Sync {
    /// List projects with counts only (no finding rows).
    async fn list_workspace(&self) -> Result<WorkspaceInventory, String>;

    /// Project metadata + targets + scans (no finding rows).
    async fn project_detail(&self, project: &str) -> Result<ProjectDetail, String>;

    /// List scans for a project (id or name).
    async fn list_scan(&self, project: &str) -> Result<ScanList, String>;

    /// Scan metadata + capped findings for that scan.
    async fn scan_detail(&self, scan_id: &str) -> Result<ScanDetail, String>;

    /// Paginated findings for a project and/or scan.
    async fn list_findings(
        &self,
        project: Option<&str>,
        scan_id: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<FindingList, String>;

    /// One finding by id, or by project + 1-based index (newest-first).
    async fn finding_detail(
        &self,
        finding_id: Option<&str>,
        project: Option<&str>,
        index: Option<usize>,
    ) -> Result<FindingDetail, String>;

    /// List targets for a project (id or name).
    async fn list_targets(&self, project: &str) -> Result<TargetList, String>;

    /// One target by id (or project + name), with clipped profile summary.
    async fn target_detail(
        &self,
        target_id: Option<&str>,
        project: Option<&str>,
        name: Option<&str>,
    ) -> Result<TargetDetail, String>;

    /// List reports for a project, or all reports when project is None/empty.
    async fn list_reports(&self, project: Option<&str>) -> Result<ReportList, String>;

    /// One report by id; content is clipped for agent context.
    async fn report_detail(&self, report_id: &str) -> Result<ReportDetail, String>;
}

pub fn clamp_findings_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(DEFAULT_FINDINGS_LIMIT)
        .clamp(1, MAX_FINDINGS_LIMIT)
}

pub fn parse_finding_index(goal: &str) -> Option<usize> {
    let g = goal.to_lowercase();
    for (_, rest) in [
        ("finding số ", g.split_once("finding số ").map(|(_, r)| r)),
        ("finding #", g.split_once("finding #").map(|(_, r)| r)),
        ("finding ", g.split_once("finding ").map(|(_, r)| r)),
        ("lỗ hổng số ", g.split_once("lỗ hổng số ").map(|(_, r)| r)),
        ("lo hong so ", g.split_once("lo hong so ").map(|(_, r)| r)),
    ] {
        if let Some(rest) = rest {
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = digits.parse::<usize>() {
                if n > 0 {
                    return Some(n);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_workspace_observation_is_project_only() {
        let inv = WorkspaceInventory {
            projects: vec![WorkspaceProjectSummary {
                id: "p1".into(),
                name: "AI".into(),
                description: None,
                findings_count: 36,
                targets_count: 2,
                scans_count: 2,
            }],
            totals: WorkspaceTotals {
                projects: 1,
                targets: 2,
                scans: 2,
                findings: 36,
                findings_truncated: 0,
            },
        };
        let obs = inv.to_observation();
        assert!(obs.contains("list_workspace OK"));
        assert!(obs.contains("name=AI"));
        assert!(!obs.contains("Findings (1-indexed"));
        assert!(inv.compact_user_reply_for_goal("project AI").contains("**AI**"));
    }

    #[test]
    fn parse_finding_index_vi_en() {
        assert_eq!(
            parse_finding_index("cho tôi finding số 1 của project AI"),
            Some(1)
        );
        assert_eq!(parse_finding_index("finding #3 please"), Some(3));
    }

    #[test]
    fn finding_list_compact_index() {
        let list = FindingList {
            project_id: Some("p1".into()),
            project_name: Some("AI".into()),
            scan_id: None,
            findings: vec![WorkspaceFindingSummary {
                id: "f1".into(),
                project_id: "p1".into(),
                scan_id: "s1".into(),
                title: "Jailbreak A".into(),
                severity: "high".into(),
                status: "open".into(),
                category: None,
            }],
            total: 1,
            offset: 0,
            limit: 20,
        };
        let reply = list.compact_finding_at_index(1).unwrap();
        assert!(reply.contains("Jailbreak A"));
        assert!(reply.contains("Finding #1"));
    }
}
