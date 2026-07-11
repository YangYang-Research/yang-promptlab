import { describe, expect, it } from "vitest";

import {
  applyWizardEntryStep,
  createInitialSession,
  createSessionForTargetScan,
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
});
