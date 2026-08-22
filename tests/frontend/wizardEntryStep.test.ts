import { describe, expect, it } from "vitest";

import {
  applyWizardEntryStep,
  createInitialSession,
  createSessionForTargetScan,
  shouldAutoStartRetry,
} from "@/features/scans/wizardState";
import type { Target } from "@/shared/types";

function sampleTarget(): Target {
  return {
    id: "target-1",
    projectId: "project-1",
    name: "Demo API",
    url: "https://example.com/v1",
    type: "api",
    providerLabel: null,
    status: "verified",
    createdAt: "2026-07-01T00:00:00.000Z",
    lastScanAt: null,
    fingerprint: null,
    tags: [],
    authType: "none",
    authKind: "none",
  };
}

describe("applyWizardEntryStep", () => {
  it("clears step 2 when creating a new target", () => {
    const session = createInitialSession("project-1");
    const next = applyWizardEntryStep(session, 2);
    expect(next.currentStep).toBe(2);
    expect(next.savedTargetId).toBeNull();
    expect(next.targetForm.url || next.targetForm.baseUrl || "").toBe("");
  });

  it("keeps existing target data when starting a new scan at step 2", () => {
    const target = sampleTarget();
    const base = createSessionForTargetScan(
      "project-1",
      target,
      { url: target.url },
      { provider: "openai", verification: { verified: true } },
      2,
    );
    base.attackPlan = { profileId: "standard" } as never;
    base.draftScanId = "draft-old";
    base.submittedScanId = "scan-old";

    const next = applyWizardEntryStep(base, 2);
    expect(next.currentStep).toBe(2);
    expect(next.savedTargetId).toBe("target-1");
    expect(next.targetForm.url || next.targetForm.baseUrl).toContain("example.com");
    expect(next.draftScanId).toBeNull();
    expect(next.submittedScanId).toBeNull();
    expect(next.attackPlan).toBeNull();
  });

  it("clears submittedScanId when entering step 4 for retry", () => {
    const session = createInitialSession("project-1");
    session.currentStep = 5;
    session.draftScanId = "failed-scan";
    session.submittedScanId = "failed-scan";

    const next = applyWizardEntryStep(session, 4);
    expect(next.currentStep).toBe(4);
    expect(next.submittedScanId).toBeNull();
    expect(next.draftScanId).toBe("failed-scan");
  });

  it("does not promote draftScanId to submittedScanId when resuming step 5", () => {
    const session = createInitialSession("project-1");
    session.currentStep = 5;
    session.draftScanId = "draft-1";
    session.submittedScanId = null;
    session.attackPlan = { profileId: "standard", categories: ["prompt-injection"] } as never;

    const next = applyWizardEntryStep(session, 5);
    expect(next.currentStep).toBe(5);
    expect(next.draftScanId).toBe("draft-1");
    expect(next.submittedScanId).toBeNull();
  });

  it("keeps an existing submittedScanId when entering step 5", () => {
    const session = createInitialSession("project-1");
    session.currentStep = 5;
    session.draftScanId = "scan-1";
    session.submittedScanId = "scan-1";

    const next = applyWizardEntryStep(session, 5);
    expect(next.currentStep).toBe(5);
    expect(next.submittedScanId).toBe("scan-1");
  });
});

describe("shouldAutoStartRetry", () => {
  it("starts Retry Scan only for terminal retryable statuses", () => {
    expect(shouldAutoStartRetry("failed")).toBe(true);
    expect(shouldAutoStartRetry("cancelled")).toBe(true);
    expect(shouldAutoStartRetry("stopped")).toBe(true);
    expect(shouldAutoStartRetry("running")).toBe(false);
    expect(shouldAutoStartRetry("paused")).toBe(false);
    expect(shouldAutoStartRetry("pending")).toBe(false);
    expect(shouldAutoStartRetry("completed")).toBe(false);
    expect(shouldAutoStartRetry(null)).toBe(false);
  });
});
