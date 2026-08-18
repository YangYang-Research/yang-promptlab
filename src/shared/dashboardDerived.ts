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
  return scan.name.startsWith("Scan (") || scan.name.startsWith("Agent Scan (");
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
    .filter(isAttackScan)
    .map((scan) => {
      const live = liveStatus.get(scan.id);
      const effectiveStatus = live?.status ?? scan.status;
      if (!RUNNING_STATUSES.has(effectiveStatus)) return null;

      const total = live?.total ?? 0;
      const completed = live?.completed ?? 0;
      return {
        id: scan.id,
        targetId: scan.targetId ?? "",
        targetName: targetName(targets, scan.targetId),
        category: "prompt_injection",
        status: effectiveStatus as AttackRun["status"],
        payloadsTotal: total > 0 ? total : 100,
        payloadsRun: completed,
        findingsCount: live?.findings_count ?? 0,
        startedAt: scan.startedAt ?? scan.createdAt,
        completedAt: scan.completedAt,
      } satisfies AttackRun;
    })
    .filter((run): run is AttackRun => run !== null);
}

export function deriveActivity(
  findings: Finding[],
  scans: ScanRun[],
  targets: Target[],
  projects: Project[],
): ActivityItem[] {
  const items: ActivityItem[] = [];

  const sortedFindings = [...findings].sort((a, b) =>
    b.discoveredAt.localeCompare(a.discoveredAt),
  );
  for (const finding of sortedFindings) {
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

    for (const retry of scan.retries ?? []) {
      items.push({
        id: `scan-retry-${scan.id}-${retry.at}`,
        type: "attack",
        message: `Retry Scan: ${scan.name}`,
        timestamp: retry.at,
      });
    }

    if (scan.status === "running" || scan.status === "paused" || scan.status === "pending") {
      items.push({
        id: `scan-active-${scan.id}`,
        type: "attack",
        message: `Scan ${scan.status}: ${scan.name}`,
        timestamp: scan.startedAt ?? scan.createdAt,
      });
      continue;
    }

    if (scan.status === "completed" || scan.status === "failed") {
      items.push({
        id: `scan-${scan.id}`,
        type: "attack",
        message: `Scan ${scan.status}: ${scan.name}`,
        timestamp: scan.completedAt ?? scan.startedAt ?? scan.createdAt,
      });
    }
  }

  for (const target of targets) {
    const project = projects.find((p) => p.id === target.projectId);
    items.push({
      id: `target-${target.id}`,
      type: "target",
      message: `Target added: ${target.name}${project ? ` (${project.name})` : ""}`,
      timestamp: target.createdAt,
    });
  }

  return items.sort((a, b) => b.timestamp.localeCompare(a.timestamp));
}
