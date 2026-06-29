import type { ScanRun } from "@/shared/types";

import { WIZARD_SCAN_STATUS } from "@/shared/ipc/scanWizard";

const draftCreateLocks = new Map<string, Promise<string>>();
const draftScanStorageKey = (projectId: string) => `promptlab:draft-scan:${projectId}`;

export function peekStoredDraftScanId(projectId: string): string | null {
  if (typeof window === "undefined" || !projectId) return null;
  try {
    return window.sessionStorage.getItem(draftScanStorageKey(projectId));
  } catch {
    return null;
  }
}

export function storeDraftScanId(projectId: string, scanId: string): void {
  if (typeof window === "undefined" || !projectId) return;
  try {
    window.sessionStorage.setItem(draftScanStorageKey(projectId), scanId);
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
    const forTarget = drafts.find((scan) => scan.targetId === targetId);
    if (forTarget) return forTarget;
  }

  return [...drafts].sort((a, b) => b.createdAt.localeCompare(a.createdAt))[0];
}

export async function resolveOrCreateDraftScanId(
  projectId: string,
  factory: () => Promise<string>,
): Promise<string> {
  const inFlight = draftCreateLocks.get(projectId);
  if (inFlight) return inFlight;

  const promise = factory().finally(() => {
    draftCreateLocks.delete(projectId);
  });
  draftCreateLocks.set(projectId, promise);
  return promise;
}
