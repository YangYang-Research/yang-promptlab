import type { ScanRun, TargetStatus } from "@/shared/types";
import { formatTimestamp } from "@/features/scans/scanDetailsHelpers";

export type TargetScanStatusLabel = "Never Scanned" | "Running" | "Completed" | "Failed";

export type TargetScanContext = {
  scanStatusLabel: TargetScanStatusLabel;
  lastScanTime: string | null;
  latestScanResult: string;
};

function isAttackScan(scan: ScanRun): boolean {
  return scan.name.startsWith("Scan (") || scan.name.startsWith("Agent Scan (");
}

function latestScan(scans: ScanRun[]): ScanRun | null {
  if (scans.length === 0) return null;
  return [...scans].sort((a, b) => b.createdAt.localeCompare(a.createdAt))[0];
}

function attackScansForTarget(targetId: string, scans: ScanRun[]): ScanRun[] {
  return scans.filter((scan) => scan.targetId === targetId && isAttackScan(scan));
}

function hasFinishedAttackScan(scans: ScanRun[]): boolean {
  return scans.some(
    (scan) =>
      scan.status === "completed" ||
      scan.status === "failed" ||
      scan.status === "cancelled",
  );
}

export function isTargetProfileVerified(profile: unknown): boolean {
  if (typeof profile !== "object" || profile === null || Array.isArray(profile)) {
    return false;
  }
  const verification = (profile as Record<string, unknown>).verification;
  if (typeof verification !== "object" || verification === null || Array.isArray(verification)) {
    return false;
  }
  return (verification as Record<string, unknown>).verified === true;
}

/**
 * pending  — not verified yet
 * verified — profile verified, no finished attack scan
 * scanned  — at least one finished attack scan
 */
export function deriveTargetStatus(
  profile: unknown,
  targetId: string,
  scans: ScanRun[],
): TargetStatus {
  if (hasFinishedAttackScan(attackScansForTarget(targetId, scans))) {
    return "scanned";
  }
  if (isTargetProfileVerified(profile)) {
    return "verified";
  }
  return "pending";
}

export function deriveTargetLastScanAt(targetId: string, scans: ScanRun[]): string | null {
  const attackScans = attackScansForTarget(targetId, scans);
  const finished = attackScans.filter(
    (scan) =>
      scan.status === "completed" ||
      scan.status === "failed" ||
      scan.status === "cancelled",
  );
  const latest = latestScan(finished.length > 0 ? finished : attackScans);
  if (!latest) return null;
  return latest.completedAt ?? latest.startedAt ?? latest.createdAt;
}

export function buildTargetScanContext(targetId: string, scans: ScanRun[]): TargetScanContext {
  const attackScans = attackScansForTarget(targetId, scans);

  const runningScan = attackScans.find(
    (scan) =>
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
