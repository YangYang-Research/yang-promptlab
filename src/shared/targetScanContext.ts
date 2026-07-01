import type { ScanRun } from "@/shared/types";
import { formatTimestamp } from "@/features/scans/scanDetailsHelpers";

export type TargetScanStatusLabel = "Never Scanned" | "Running" | "Completed" | "Failed";

export type TargetScanContext = {
  scanStatusLabel: TargetScanStatusLabel;
  lastScanTime: string | null;
  latestScanResult: string;
};

function isAttackScan(scan: ScanRun): boolean {
  return scan.name.startsWith("Scan (");
}

function latestScan(scans: ScanRun[]): ScanRun | null {
  if (scans.length === 0) return null;
  return [...scans].sort((a, b) => b.createdAt.localeCompare(a.createdAt))[0];
}

export function buildTargetScanContext(targetId: string, scans: ScanRun[]): TargetScanContext {
  const attackScans = scans.filter((scan) => scan.targetId === targetId && isAttackScan(scan));

  const runningScan = attackScans.find((scan) =>
    scan.status === "running" || scan.status === "paused" || scan.status === "pending",
  );
  const latestAttack = latestScan(attackScans);

  if (runningScan) {
    return {
      scanStatusLabel: "Running",
      lastScanTime: runningScan.startedAt ?? runningScan.createdAt,
      latestScanResult: `${runningScan.status} · ${runningScan.name}`,
    };
  }

  if (!latestAttack) {
    return {
      scanStatusLabel: "Never Scanned",
      lastScanTime: null,
      latestScanResult: "No scans recorded",
    };
  }

  const scanStatusLabel: TargetScanStatusLabel =
    latestAttack.status === "failed" || latestAttack.status === "cancelled"
      ? "Failed"
      : latestAttack.status === "completed"
        ? "Completed"
        : "Running";

  return {
    scanStatusLabel,
    lastScanTime: latestAttack.completedAt ?? latestAttack.startedAt ?? latestAttack.createdAt,
    latestScanResult: `${latestAttack.status} · ${latestAttack.name}`,
  };
}

export function formatTargetTimestamp(value: string | null): string {
  return value ? formatTimestamp(value) : "—";
}
