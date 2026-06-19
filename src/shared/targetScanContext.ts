import type { ScanRun } from "@/shared/types";
import { formatTimestamp } from "@/features/scans/scanDetailsHelpers";

export type TargetScanStatusLabel = "Never Scanned" | "Running" | "Completed" | "Failed";

export type TargetScanContext = {
  scanStatusLabel: TargetScanStatusLabel;
  lastScanTime: string | null;
  latestDiscoveryTime: string | null;
  latestScanResult: string;
};

function isAttackScan(scan: ScanRun): boolean {
  return scan.name.startsWith("Scan (");
}

function isDiscoveryScan(scan: ScanRun): boolean {
  return scan.name.startsWith("Discovery:");
}

function latestScan(scans: ScanRun[]): ScanRun | null {
  if (scans.length === 0) return null;
  return [...scans].sort((a, b) => b.createdAt.localeCompare(a.createdAt))[0];
}

export function buildTargetScanContext(targetId: string, scans: ScanRun[]): TargetScanContext {
  const targetScans = scans.filter((scan) => scan.targetId === targetId);
  const attackScans = targetScans.filter(isAttackScan);
  const discoveryScans = targetScans.filter(isDiscoveryScan);

  const runningScan = attackScans.find((scan) =>
    scan.status === "running" || scan.status === "paused" || scan.status === "pending",
  );
  const latestAttack = latestScan(attackScans);
  const latestDiscovery = latestScan(discoveryScans);

  if (runningScan) {
    return {
      scanStatusLabel: "Running",
      lastScanTime: runningScan.startedAt ?? runningScan.createdAt,
      latestDiscoveryTime: latestDiscovery?.completedAt ?? latestDiscovery?.createdAt ?? null,
      latestScanResult: `${runningScan.status} · ${runningScan.name}`,
    };
  }

  if (!latestAttack && !latestDiscovery) {
    return {
      scanStatusLabel: "Never Scanned",
      lastScanTime: null,
      latestDiscoveryTime: null,
      latestScanResult: "No scans recorded",
    };
  }

  const scanStatusLabel: TargetScanStatusLabel =
    latestAttack?.status === "failed" || latestAttack?.status === "cancelled"
      ? "Failed"
      : latestAttack?.status === "completed"
        ? "Completed"
        : latestAttack
          ? "Running"
          : "Never Scanned";

  return {
    scanStatusLabel,
    lastScanTime: latestAttack?.completedAt ?? latestAttack?.startedAt ?? null,
    latestDiscoveryTime: latestDiscovery?.completedAt ?? latestDiscovery?.createdAt ?? null,
    latestScanResult: latestAttack
      ? `${latestAttack.status} · ${latestAttack.name}`
      : latestDiscovery
        ? `Discovery ${latestDiscovery.status}`
        : "No attack scans yet",
  };
}

export function formatTargetTimestamp(value: string | null): string {
  return value ? formatTimestamp(value) : "—";
}
