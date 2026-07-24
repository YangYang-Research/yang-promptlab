import type { ScanRun } from "@/shared/types";

import { WIZARD_SCAN_STATUS } from "@/shared/ipc/scanWizard";

const draftCreateLocks = new Map<string, Promise<string>>();

function draftLockKey(projectId: string, targetId?: string | null): string {
  const target = targetId?.trim() ?? "";
  return target ? `${projectId}:${target}` : projectId;
}

const draftScanStorageKey = (projectId: string, targetId?: string | null) =>
  `promptlab:draft-scan:${draftLockKey(projectId, targetId)}`;

export function peekStoredDraftScanId(
  projectId: string,
  targetId?: string | null,
): string | null {
  if (typeof window === "undefined" || !projectId) return null;
  try {
    return window.sessionStorage.getItem(draftScanStorageKey(projectId, targetId));
  } catch {
    return null;
  }
}

export function storeDraftScanId(
  projectId: string,
  scanId: string,
  targetId?: string | null,
): void {
  if (typeof window === "undefined" || !projectId) return;
  try {
    window.sessionStorage.setItem(draftScanStorageKey(projectId, targetId), scanId);
  } catch {
    // Ignore quota errors.
  }
}

export function findWizardDraftScan(
  scans: ScanRun[],
  projectId: string,
  targetId?: string | null,
): ScanRun | null {
  const drafts = scans.filter(
    (scan) => scan.projectId === projectId && scan.status === WIZARD_SCAN_STATUS,
  );
  if (drafts.length === 0) return null;

  if (targetId) {
    // Never fall back to another target's draft when a target is specified.
    return drafts.find((scan) => scan.targetId === targetId) ?? null;
  }

  return [...drafts].sort((a, b) => b.createdAt.localeCompare(a.createdAt))[0];
}

export async function resolveOrCreateDraftScanId(
  projectId: string,
  factory: () => Promise<string>,
  targetId?: string | null,
): Promise<string> {
  const key = draftLockKey(projectId, targetId);
  const inFlight = draftCreateLocks.get(key);
  if (inFlight) return inFlight;

  const promise = factory().finally(() => {
    draftCreateLocks.delete(key);
  });
  draftCreateLocks.set(key, promise);
  return promise;
}

/** Whether an existing wizard draft may be reused for this URL entry. */
export function canReuseWizardDraft(params: {
  draftScanId: string | null;
  sessionTargetId: string | null;
  lockedTargetId: string;
  entryStep: number | null;
  draftScanTargetId?: string | null;
}): boolean {
  if (!params.draftScanId) return false;
  // Explicit "New Scan" from a target must start a fresh draft for that target.
  if (params.entryStep === 2 && params.lockedTargetId) return false;
  if (!params.lockedTargetId) return true;

  if (params.draftScanTargetId) {
    return params.draftScanTargetId === params.lockedTargetId;
  }
  if (params.sessionTargetId) {
    return params.sessionTargetId === params.lockedTargetId;
  }
  // Unknown binding — do not attach a foreign draft to this target.
  return false;
}
