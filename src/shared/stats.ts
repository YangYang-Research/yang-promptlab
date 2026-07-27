import type {
  DashboardStats,
  Finding,
  LocalModel,
  Project,
  ScanRun,
  Severity,
  Target,
} from "@/shared/types";

const SEVERITY_ORDER: Severity[] = ["critical", "high", "medium", "low", "info"];

const SCANNING_STATUSES = new Set(["running", "paused", "pending"]);

function isAttackScan(scan: ScanRun): boolean {
  return scan.name.startsWith("Scan (") || scan.name.startsWith("Agent Scan (");
}

function latestScanPerTarget(scans: ScanRun[]): Map<string, ScanRun> {
  const byTarget = new Map<string, ScanRun>();
  for (const scan of scans) {
    if (!scan.targetId) continue;
    const existing = byTarget.get(scan.targetId);
    if (!existing) {
      byTarget.set(scan.targetId, scan);
      continue;
    }
    const existingAt = existing.startedAt ?? existing.createdAt;
    const candidateAt = scan.startedAt ?? scan.createdAt;
    if (candidateAt > existingAt) {
      byTarget.set(scan.targetId, scan);
    }
  }
  return byTarget;
}


function countScanningTargets(scans: ScanRun[]): number {
  const latest = latestScanPerTarget(scans);
  let count = 0;
  for (const scan of latest.values()) {
    if (!isAttackScan(scan)) continue;
    if (SCANNING_STATUSES.has(scan.status)) {
      count += 1;
    }
  }
  return count;
}

export function computeDashboardStats(
  projects: Project[],
  targets: Target[],
  findings: Finding[],
  scans: ScanRun[],
  models: LocalModel[],
): DashboardStats {
  return {
    projects: projects.length,
    activeProjects: projects.filter((project) => project.status === "active").length,
    targets: targets.length,
    scanningTargets: countScanningTargets(scans),
    openFindings: findings.filter((f) => f.status === "open" || f.status === "confirmed").length,
    criticalFindings: findings.filter((f) => f.severity === "critical").length,
    runningScans: scans.filter((s) => s.status === "running").length,
    installedModels: models.filter((m) => m.status === "installed").length,
    downloadingModels: models.filter((m) => m.status === "downloading").length,
  };
}

export function severityCounts(findings: Finding[]): Record<Severity, number> {
  const counts: Record<Severity, number> = {
    critical: 0,
    high: 0,
    medium: 0,
    low: 0,
    info: 0,
  };
  for (const f of findings) {
    counts[f.severity] = (counts[f.severity] ?? 0) + 1;
  }
  return counts;
}

export type SeverityCountSlice = {
  severity: Severity;
  label: string;
  count: number;
};

export function severityCountSeries(findings: Finding[]): SeverityCountSlice[] {
  const counts = severityCounts(findings);
  return SEVERITY_ORDER.map((severity) => ({
    severity,
    label: severity.charAt(0).toUpperCase() + severity.slice(1),
    count: counts[severity],
  }));
}
