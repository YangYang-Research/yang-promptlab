import type { ScanStatusDto } from "@/shared/ipc";
import type {
  ActivityItem,
  AttackRun,
  Finding,
  Project,
  ScanRun,
  Target,
} from "@/shared/types";

const RUNNING_STATUSES = new Set(["running", "paused", "pending"]);

function isAttackScan(scan: ScanRun): boolean {
  return scan.name.startsWith("Scan (");
}

function targetName(targets: Target[], targetId: string | null): string {
  if (!targetId) return "Unknown target";
  return targets.find((t) => t.id === targetId)?.name ?? "Unknown target";
}

export function deriveAttackRuns(
  scans: ScanRun[],
  targets: Target[],
  liveStatus: Map<string, ScanStatusDto>,
): AttackRun[] {
  return scans
    .filter((scan) => isAttackScan(scan) && RUNNING_STATUSES.has(scan.status))
    .map((scan) => {
      const live = liveStatus.get(scan.id);
      const total = live?.total ?? 0;
      const completed = live?.completed ?? 0;
      return {
        id: scan.id,
        targetId: scan.targetId ?? "",
        targetName: targetName(targets, scan.targetId),
        category: "prompt_injection",
        status: scan.status,
        payloadsTotal: total > 0 ? total : 100,
        payloadsRun: completed,
        findingsCount: live?.findings_count ?? 0,
        startedAt: scan.startedAt ?? scan.createdAt,
        completedAt: scan.completedAt,
      };
    });
}

export function deriveActivity(
  findings: Finding[],
  scans: ScanRun[],
  targets: Target[],
  projects: Project[],
): ActivityItem[] {
  const items: ActivityItem[] = [];

  for (const finding of findings.slice(0, 12)) {
    items.push({
      id: `finding-${finding.id}`,
      type: "finding",
      message: `${finding.title} on ${finding.targetName || "target"}`,
      timestamp: finding.discoveredAt,
      severity: finding.severity,
    });
  }

  for (const scan of scans) {
    if (!isAttackScan(scan)) continue;
    if (scan.status !== "completed" && scan.status !== "failed") continue;
    items.push({
      id: `scan-${scan.id}`,
      type: "attack",
      message: `Scan ${scan.status}: ${scan.name}`,
      timestamp: scan.completedAt ?? scan.startedAt ?? scan.createdAt,
    });
  }

  for (const target of targets.slice(0, 5)) {
    const project = projects.find((p) => p.id === target.projectId);
    items.push({
      id: `target-${target.id}`,
      type: "target",
      message: `Target added: ${target.name}${project ? ` (${project.name})` : ""}`,
      timestamp: scanTimestampForTarget(target.id, scans) ?? new Date().toISOString(),
    });
  }

  return items
    .sort((a, b) => b.timestamp.localeCompare(a.timestamp))
    .slice(0, 12);
}

function scanTimestampForTarget(targetId: string, scans: ScanRun[]): string | null {
  const related = scans
    .filter((s) => s.targetId === targetId)
    .sort((a, b) => (b.startedAt ?? b.createdAt).localeCompare(a.startedAt ?? a.createdAt));
  const latest = related[0];
  return latest ? latest.startedAt ?? latest.createdAt : null;
}
