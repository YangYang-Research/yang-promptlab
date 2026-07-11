import type { WizardStepId } from "@/features/scans/wizardSteps";
import type { ScanRun } from "@/shared/types";

import { buildTargetScanContext } from "./targetScanContext";

export type TargetScanAction =
  | { kind: "setup"; step: WizardStepId; scanId?: string }
  | { kind: "view_scan"; scanId: string }
  | { kind: "view_report"; scanId: string }
  | { kind: "retry"; scanId: string; step: WizardStepId };

function findDraftScan(targetId: string, projectId: string, scans: ScanRun[]): ScanRun | null {
  return (
    scans.find(
      (scan) =>
        scan.targetId === targetId &&
        scan.projectId === projectId &&
        scan.status === "draft",
    ) ?? null
  );
}

function isAttackScan(scan: ScanRun): boolean {
  return scan.name.startsWith("Scan (") || scan.name.startsWith("Agent Scan (");
}

function latestAttackScan(targetId: string, scans: ScanRun[]): ScanRun | null {
  const attackScans = scans.filter((scan) => scan.targetId === targetId && isAttackScan(scan));
  if (attackScans.length === 0) return null;
  return [...attackScans].sort((a, b) => b.createdAt.localeCompare(a.createdAt))[0];
}

function runningAttackScan(targetId: string, scans: ScanRun[]): ScanRun | null {
  return (
    scans.find(
      (scan) =>
        scan.targetId === targetId &&
        isAttackScan(scan) &&
        (scan.status === "running" || scan.status === "paused" || scan.status === "pending"),
    ) ?? null
  );
}

export type WizardResumeInput = {
  savedTargetId: string | null;
  selectedProjectId: string;
  currentStep: WizardStepId;
  profileVerified: boolean;
  attackPlanGenerated: boolean;
  submittedScanId: string | null;
};

export function inferWizardResumeStep(input: WizardResumeInput): WizardStepId {
  if (input.submittedScanId) {
    return input.currentStep >= 6 ? 6 : 5;
  }

  const maxReachable = inferMaxReachableWizardStep(input);
  if (input.currentStep >= 1 && input.currentStep <= 5) {
    return input.currentStep <= maxReachable ? input.currentStep : maxReachable;
  }

  return maxReachable;
}

/** Furthest step allowed without skipping required wizard work. */
export function inferMaxReachableWizardStep(input: WizardResumeInput): WizardStepId {
  if (!input.savedTargetId && !input.selectedProjectId) {
    return 1;
  }
  if (!input.savedTargetId) {
    return 2;
  }
  if (!input.profileVerified) {
    return 3;
  }
  if (!input.attackPlanGenerated) {
    return 4;
  }
  return 5;
}

export function wizardSessionMatchesTarget(
  session: WizardResumeInput,
  targetId: string,
  projectId: string,
): boolean {
  if (session.savedTargetId !== targetId) return false;
  if (!projectId) return true;
  return !session.selectedProjectId || session.selectedProjectId === projectId;
}

export function isWizardSetupIncomplete(
  session: WizardResumeInput,
  targetId: string,
  projectId: string,
): boolean {
  if (!wizardSessionMatchesTarget(session, targetId, projectId)) {
    return false;
  }
  if (session.submittedScanId) {
    return false;
  }
  return session.currentStep < 6;
}

export function resolveTargetScanAction(
  targetId: string,
  projectId: string,
  scans: ScanRun[],
  wizardSession: WizardResumeInput | null,
): TargetScanAction {
  const context = buildTargetScanContext(targetId, scans);
  const latestAttack = latestAttackScan(targetId, scans);
  const running = runningAttackScan(targetId, scans);

  if (running) {
    return { kind: "view_scan", scanId: running.id };
  }

  if (context.scanStatusLabel === "Completed" && latestAttack) {
    return { kind: "view_report", scanId: latestAttack.id };
  }

  if (
    context.scanStatusLabel === "Failed" &&
    latestAttack &&
    (latestAttack.status === "failed" || latestAttack.status === "cancelled")
  ) {
    return { kind: "retry", scanId: latestAttack.id, step: 4 };
  }

  if (wizardSession && isWizardSetupIncomplete(wizardSession, targetId, projectId)) {
    const draft = findDraftScan(targetId, projectId, scans);
    return {
      kind: "setup",
      step: inferWizardResumeStep(wizardSession),
      scanId: draft?.id,
    };
  }

  const draft = findDraftScan(targetId, projectId, scans);
  return {
    kind: "setup",
    step:
      wizardSession && wizardSessionMatchesTarget(wizardSession, targetId, projectId)
        ? inferWizardResumeStep(wizardSession)
        : 3,
    scanId: draft?.id,
  };
}
