import { describe, expect, it } from "vitest";

import {
  inferWizardResumeStep,
  isWizardSetupIncomplete,
  resolveTargetScanAction,
  type WizardResumeInput,
} from "@/shared/targetScanAction";
import type { ScanRun } from "@/shared/types";

function scan(overrides: Partial<ScanRun> & Pick<ScanRun, "id" | "status">): ScanRun {
  return {
    projectId: "project-1",
    targetId: "target-1",
    name: "Scan (standard)",
    createdAt: "2026-01-01T00:00:00Z",
    startedAt: "2026-01-01T00:00:00Z",
    completedAt: null,
    ...overrides,
  };
}

function wizardInput(overrides: Partial<WizardResumeInput> = {}): WizardResumeInput {
  return {
    savedTargetId: "target-1",
    selectedProjectId: "project-1",
    currentStep: 3,
    profileVerified: false,
    attackPlanGenerated: false,
    draftScanId: null,
    submittedScanId: null,
    ...overrides,
  };
}

describe("inferWizardResumeStep", () => {
  it("returns step 4 when verified but no attack plan", () => {
    expect(
      inferWizardResumeStep(
        wizardInput({
          profileVerified: true,
          currentStep: 4,
        }),
      ),
    ).toBe(4);
  });

  it("returns step 5 when plan exists but scan not submitted", () => {
    expect(
      inferWizardResumeStep(
        wizardInput({
          profileVerified: true,
          attackPlanGenerated: true,
          currentStep: 5,
        }),
      ),
    ).toBe(5);
  });

  it("stays on step 4 when attack plan exists but user is still reviewing", () => {
    expect(
      inferWizardResumeStep(
        wizardInput({
          profileVerified: true,
          attackPlanGenerated: true,
          currentStep: 4,
        }),
      ),
    ).toBe(4);
  });

  it("clamps ahead-of-prerequisites progress back to the reachable step", () => {
    expect(
      inferWizardResumeStep(
        wizardInput({
          profileVerified: false,
          attackPlanGenerated: false,
          currentStep: 5,
        }),
      ),
    ).toBe(3);
  });
});

describe("resolveTargetScanAction", () => {
  it("shows view scan for running scans", () => {
    const action = resolveTargetScanAction("target-1", "project-1", [
      scan({ id: "scan-running", status: "running" }),
    ], null);

    expect(action).toEqual({ kind: "view_scan", scanId: "scan-running" });
  });

  it("shows view report for completed scans", () => {
    const action = resolveTargetScanAction("target-1", "project-1", [
      scan({ id: "scan-1", status: "completed", completedAt: "2026-01-01T01:00:00Z" }),
    ], null);

    expect(action).toEqual({ kind: "view_report", scanId: "scan-1" });
  });

  it("shows retry for failed scans", () => {
    const action = resolveTargetScanAction("target-1", "project-1", [
      scan({ id: "scan-2", status: "failed" }),
    ], null);

    expect(action).toEqual({ kind: "retry", scanId: "scan-2", step: 4 });
  });

  it("resumes setup at the wizard step when session is incomplete", () => {
    const action = resolveTargetScanAction(
      "target-1",
      "project-1",
      [],
      wizardInput({ currentStep: 4, profileVerified: true, attackPlanGenerated: true }),
    );

    expect(action).toEqual({ kind: "setup", step: 4, scanId: undefined });
  });

  it("ignores orphaned wizard session after its draft scan was deleted", () => {
    const action = resolveTargetScanAction(
      "target-1",
      "project-1",
      [],
      wizardInput({
        currentStep: 5,
        profileVerified: true,
        attackPlanGenerated: true,
        draftScanId: "deleted-draft",
      }),
    );

    expect(action).toEqual({ kind: "setup", step: 2, scanId: undefined });
  });

  it("detects incomplete setup on step 3", () => {
    expect(
      isWizardSetupIncomplete(
        wizardInput({ currentStep: 3 }),
        "target-1",
        "project-1",
      ),
    ).toBe(true);
  });
});
