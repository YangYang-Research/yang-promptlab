import type {
  DashboardStats,
  DiscoveryJob,
  Finding,
  LocalModel,
  Project,
  Severity,
  Target,
} from "@/shared/types";

export function computeDashboardStats(
  projects: Project[],
  targets: Target[],
  findings: Finding[],
  discoveryJobs: DiscoveryJob[],
  models: LocalModel[],
): DashboardStats {
  return {
    projects: projects.length,
    targets: targets.length,
    openFindings: findings.filter((f) => f.status === "open" || f.status === "confirmed").length,
    criticalFindings: findings.filter((f) => f.severity === "critical").length,
    runningScans: discoveryJobs.filter((j) => j.status === "running").length,
    installedModels: models.filter((m) => m.status === "installed").length,
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
