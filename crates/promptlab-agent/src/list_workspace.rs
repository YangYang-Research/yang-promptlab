//! Host-backed workspace inventory tool for Yazg ReAct.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Compact finding row for workspace listings.
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

/// Scan row for workspace listings.
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

/// Target row for workspace listings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTargetSummary {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub target_type: String,
}

/// Project row for workspace listings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceProjectSummary {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Total findings for this project (accurate even when the findings list is truncated).
    #[serde(default)]
    pub findings_count: usize,
}

/// Totals for the inventory snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTotals {
    pub projects: usize,
    pub targets: usize,
    pub scans: usize,
    pub findings: usize,
    /// Findings omitted from the list due to the display cap.
    #[serde(default)]
    pub findings_truncated: usize,
}

/// DB-backed workspace inventory returned to Yazg as an Observation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInventory {
    pub projects: Vec<WorkspaceProjectSummary>,
    pub targets: Vec<WorkspaceTargetSummary>,
    pub scans: Vec<WorkspaceScanSummary>,
    pub findings: Vec<WorkspaceFindingSummary>,
    pub totals: WorkspaceTotals,
}

impl WorkspaceInventory {
    /// Human-readable observation for the ReAct transcript.
    pub fn to_observation(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "ListWorkspaceTool OK — projects={} targets={} scans={} findings={} (listed {} findings{})",
            self.totals.projects,
            self.totals.targets,
            self.totals.scans,
            self.totals.findings,
            self.findings.len(),
            if self.totals.findings_truncated > 0 {
                format!(", truncated {}", self.totals.findings_truncated)
            } else {
                String::new()
            }
        ));

        if self.projects.is_empty() {
            lines.push("Projects: (none)".into());
        } else {
            lines.push("Projects:".into());
            for project in &self.projects {
                let desc = project
                    .description
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("-");
                lines.push(format!(
                    "  - id={} name={} description={}",
                    project.id, project.name, desc
                ));
            }
        }

        if self.targets.is_empty() {
            lines.push("Targets: (none)".into());
        } else {
            lines.push("Targets:".into());
            for target in &self.targets {
                lines.push(format!(
                    "  - id={} project={} name={} type={}",
                    target.id, target.project_id, target.name, target.target_type
                ));
            }
        }

        if self.scans.is_empty() {
            lines.push("Scans: (none)".into());
        } else {
            lines.push("Scans:".into());
            for scan in &self.scans {
                lines.push(format!(
                    "  - id={} project={} name={} status={} target={}",
                    scan.id,
                    scan.project_id,
                    scan.name,
                    scan.status,
                    scan.target_id.as_deref().unwrap_or("-")
                ));
            }
        }

        if self.findings.is_empty() {
            lines.push("Findings: (none)".into());
        } else {
            lines.push("Findings (1-indexed; use this order for 'finding #N' / 'finding số N'):".into());
            for (idx, finding) in self.findings.iter().enumerate() {
                lines.push(format!(
                    "  {}. id={} project={} scan={} severity={} status={} title={}",
                    idx + 1,
                    finding.id,
                    finding.project_id,
                    finding.scan_id,
                    finding.severity,
                    finding.status,
                    finding.title
                ));
            }
        }

        lines.join("\n")
    }

    /// User-facing final reply after a successful list_workspace Observation.
    pub fn to_user_reply(&self) -> String {
        self.to_user_reply_for_goal("")
    }

    /// Prefer a project-scoped finding count when the goal names a project.
    ///
    /// Prefer [`Self::compact_user_reply_for_goal`] for chat — this full dump is
    /// a legacy salvage and feels hardcoded in the UI.
    pub fn to_user_reply_for_goal(&self, goal: &str) -> String {
        let mut lines = Vec::new();
        if let Some((project, count)) = self.project_finding_hit(goal) {
            lines.push(format!(
                "Project **{}** has **{}** finding{}.",
                project.name,
                count,
                if count == 1 { "" } else { "s" }
            ));
            lines.push(String::new());
        }

        lines.push(format!(
            "Workspace inventory from the local database:\n- Projects: {}\n- Targets: {}\n- Scans: {}\n- Findings: {}{}",
            self.totals.projects,
            self.totals.targets,
            self.totals.scans,
            self.totals.findings,
            if self.totals.findings_truncated > 0 {
                format!(" (showing {}, truncated {})", self.findings.len(), self.totals.findings_truncated)
            } else {
                String::new()
            }
        ));

        lines.push(String::new());
        lines.push("Findings by project:".into());
        if self.projects.is_empty() {
            lines.push("- (none)".into());
        } else {
            for project in &self.projects {
                let count = project.findings_count;
                lines.push(format!(
                    "- {}: {} finding{}",
                    project.name,
                    count,
                    if count == 1 { "" } else { "s" }
                ));
            }
        }

        lines.push(String::new());
        lines.push("Projects:".into());
        if self.projects.is_empty() {
            lines.push("- (none)".into());
        } else {
            for project in &self.projects {
                lines.push(format!("- {} (`{}`)", project.name, project.id));
            }
        }

        lines.push(String::new());
        lines.push("Targets:".into());
        if self.targets.is_empty() {
            lines.push("- (none)".into());
        } else {
            for target in &self.targets {
                lines.push(format!(
                    "- {} (`{}`, type={}, project=`{}`)",
                    target.name, target.id, target.target_type, target.project_id
                ));
            }
        }

        lines.push(String::new());
        lines.push("Scans:".into());
        if self.scans.is_empty() {
            lines.push("- (none)".into());
        } else {
            for scan in &self.scans {
                lines.push(format!(
                    "- {} (`{}`, status={}, project=`{}`)",
                    scan.name, scan.id, scan.status, scan.project_id
                ));
            }
        }

        lines.push(String::new());
        lines.push("Findings:".into());
        if self.findings.is_empty() {
            lines.push("- (none)".into());
        } else {
            for finding in &self.findings {
                let project_label = self
                    .projects
                    .iter()
                    .find(|project| project.id == finding.project_id)
                    .map(|project| project.name.as_str())
                    .unwrap_or(finding.project_id.as_str());
                lines.push(format!(
                    "- [{}] {} (`{}`, project={}, status={})",
                    finding.severity, finding.title, finding.id, project_label, finding.status
                ));
            }
        }

        lines.join("\n")
    }

    /// Short markdown answer tailored to the user goal (no full inventory dump).
    pub fn compact_user_reply_for_goal(&self, goal: &str) -> String {
        if let Some(n) = parse_finding_index(goal) {
            return self.finding_at_index(n).unwrap_or_else(|| {
                format!(
                    "No finding #{n} in the listed results (showing {} of {} findings).",
                    self.findings.len(),
                    self.totals.findings
                )
            });
        }

        if let Some((project, count)) = self.project_finding_hit(goal) {
            let targets: Vec<&str> = self
                .targets
                .iter()
                .filter(|t| t.project_id == project.id)
                .map(|t| t.name.as_str())
                .collect();
            let scans: Vec<String> = self
                .scans
                .iter()
                .filter(|s| s.project_id == project.id)
                .map(|s| format!("{} ({})", s.name, s.status))
                .collect();
            return format!(
                "**{}**\n\n- Project id: `{}`\n- Findings: **{}**\n- Targets ({}): {}\n- Scans ({}): {}\n\nAsk for a specific finding (e.g. finding #1) for details.",
                project.name,
                project.id,
                count,
                targets.len(),
                if targets.is_empty() {
                    "(none)".into()
                } else {
                    targets.join(", ")
                },
                scans.len(),
                if scans.is_empty() {
                    "(none)".into()
                } else {
                    scans.join(", ")
                }
            );
        }

        format!(
            "Workspace: **{}** projects, **{}** targets, **{}** scans, **{}** findings.",
            self.totals.projects, self.totals.targets, self.totals.scans, self.totals.findings
        )
    }

    fn finding_at_index(&self, one_based: usize) -> Option<String> {
        if one_based == 0 {
            return None;
        }
        let finding = self.findings.get(one_based - 1)?;
        let project_label = self
            .projects
            .iter()
            .find(|project| project.id == finding.project_id)
            .map(|project| project.name.as_str())
            .unwrap_or(finding.project_id.as_str());
        Some(format!(
            "### Finding #{one_based}\n\n- **Title:** {}\n- **Severity:** {}\n- **Status:** {}\n- **Project:** {}\n- **Id:** `{}`\n- **Scan:** `{}`",
            finding.title,
            finding.severity,
            finding.status,
            project_label,
            finding.id,
            finding.scan_id
        ))
    }

    /// Match a project named in the user goal and return its finding count (from listed rows).
    fn project_finding_hit(&self, goal: &str) -> Option<(&WorkspaceProjectSummary, usize)> {
        if goal.trim().is_empty() || self.projects.is_empty() {
            return None;
        }
        let g = goal.to_lowercase();
        let mut ranked: Vec<(&WorkspaceProjectSummary, usize)> = self
            .projects
            .iter()
            .filter(|project| {
                let name = project.name.trim();
                !name.is_empty() && g.contains(&name.to_lowercase())
            })
            .map(|project| {
                (
                    project,
                    if project.findings_count > 0 {
                        project.findings_count
                    } else {
                        self.findings
                            .iter()
                            .filter(|finding| finding.project_id == project.id)
                            .count()
                    },
                )
            })
            .collect();
        // Prefer longer name matches ("AI Lab" over "AI").
        ranked.sort_by(|a, b| b.0.name.len().cmp(&a.0.name.len()));
        ranked.into_iter().next()
    }

    /// True when a finish reply already presents this inventory (avoid clobbering good LLM text).
    pub fn reply_covers_inventory(&self, reply: &str) -> bool {
        let lower = reply.to_ascii_lowercase();
        if lower.len() < 40 {
            return false;
        }
        let has_totals = lower.contains("project")
            && lower.contains("target")
            && (lower.contains("scan") || lower.contains("finding"));
        if !has_totals {
            return false;
        }
        // At least one concrete project name or id, unless the workspace is empty.
        if self.projects.is_empty() {
            return lower.contains("none") || lower.contains("0 project");
        }
        self.projects.iter().any(|project| {
            (!project.name.trim().is_empty() && reply.contains(&project.name))
                || reply.contains(&project.id)
        })
    }
}

/// Host implements SQLite reads for workspace inventory.
#[async_trait]
pub trait WorkspaceTools: Send + Sync {
    async fn list_workspace(&self) -> Result<WorkspaceInventory, String>;
}

fn parse_finding_index(goal: &str) -> Option<usize> {
    let g = goal.to_lowercase();
    // "finding số 1", "finding #1", "finding 1", "lỗ hổng số 1"
    for (label, rest) in [
        ("finding số ", g.split_once("finding số ").map(|(_, r)| r)),
        ("finding #", g.split_once("finding #").map(|(_, r)| r)),
        ("finding ", g.split_once("finding ").map(|(_, r)| r)),
        ("lỗ hổng số ", g.split_once("lỗ hổng số ").map(|(_, r)| r)),
        ("lo hong so ", g.split_once("lo hong so ").map(|(_, r)| r)),
    ] {
        let _ = label;
        if let Some(rest) = rest {
            let digits: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
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
    fn vague_finish_does_not_cover_inventory() {
        let inventory = WorkspaceInventory {
            projects: vec![WorkspaceProjectSummary {
                id: "p1".into(),
                name: "AI".into(),
                description: None,
                findings_count: 0,
            }],
            targets: vec![WorkspaceTargetSummary {
                id: "t1".into(),
                project_id: "p1".into(),
                name: "demo".into(),
                target_type: "openai_compatible".into(),
            }],
            scans: Vec::new(),
            findings: Vec::new(),
            totals: WorkspaceTotals {
                projects: 1,
                targets: 1,
                scans: 0,
                findings: 0,
                findings_truncated: 0,
            },
        };
        let vague =
            "You have listed existing projects. Select a target or create a project.";
        assert!(!inventory.reply_covers_inventory(vague));
        assert!(inventory.reply_covers_inventory(&inventory.to_user_reply()));
        assert!(inventory.to_user_reply().contains("AI"));
        assert!(inventory.to_user_reply().contains("demo"));
    }

    #[test]
    fn project_scoped_reply_leads_with_finding_count() {
        let inventory = WorkspaceInventory {
            projects: vec![WorkspaceProjectSummary {
                id: "p1".into(),
                name: "AI".into(),
                description: None,
                findings_count: 36,
            }],
            targets: Vec::new(),
            scans: Vec::new(),
            findings: Vec::new(),
            totals: WorkspaceTotals {
                projects: 1,
                targets: 0,
                scans: 0,
                findings: 36,
                findings_truncated: 0,
            },
        };
        let reply = inventory
            .to_user_reply_for_goal("cho tôi số lỗ hổng của project AI");
        assert!(
            reply.starts_with("Project **AI** has **36** findings."),
            "reply={reply}"
        );
    }

    #[test]
    fn compact_reply_answers_finding_index() {
        let inventory = WorkspaceInventory {
            projects: vec![WorkspaceProjectSummary {
                id: "p1".into(),
                name: "AI".into(),
                description: None,
                findings_count: 2,
            }],
            targets: Vec::new(),
            scans: Vec::new(),
            findings: vec![
                WorkspaceFindingSummary {
                    id: "f1".into(),
                    project_id: "p1".into(),
                    scan_id: "s1".into(),
                    severity: "high".into(),
                    status: "open".into(),
                    title: "Jailbreak A".into(),
                    category: None,
                },
                WorkspaceFindingSummary {
                    id: "f2".into(),
                    project_id: "p1".into(),
                    scan_id: "s1".into(),
                    severity: "medium".into(),
                    status: "open".into(),
                    title: "Injection B".into(),
                    category: None,
                },
            ],
            totals: WorkspaceTotals {
                projects: 1,
                targets: 0,
                scans: 0,
                findings: 2,
                findings_truncated: 0,
            },
        };
        let reply = inventory.compact_user_reply_for_goal("cho tôi finding số 1 của project AI");
        assert!(reply.contains("Finding #1"), "reply={reply}");
        assert!(reply.contains("Jailbreak A"), "reply={reply}");
        assert!(!reply.contains("Injection B"), "reply={reply}");
        assert!(!reply.contains("Workspace inventory from the local database"));
    }

    #[test]
    fn parse_finding_index_vi_en() {
        assert_eq!(parse_finding_index("cho tôi finding số 1 của project AI"), Some(1));
        assert_eq!(parse_finding_index("finding #3 please"), Some(3));
    }
}
