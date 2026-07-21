import { describe, expect, it } from "vitest";

import { createInitialTargetProfile } from "@/features/scans/targetProfile";
import { canNavigateToStep, canProceedFromStep, type WizardDraft } from "@/features/scans/wizardSteps";

function baseDraft(overrides: Partial<WizardDraft> = {}): WizardDraft {
  return {
    projectId: "project-1",
    target: null,
    targetProfile: createInitialTargetProfile(),
    targetForm: {} as WizardDraft["targetForm"],
    profileVerified: false,
    attackPlan: null,
    attackPlanGenerated: false,
    submittedScanId: null,
    ...overrides,
  };
}

describe("canProceedFromStep", () => {
  it("enables step 2 next when profile is valid even without a saved target", () => {
    expect(canProceedFromStep(2, baseDraft())).toBe(true);
  });

  it("blocks step 2 next when profile validation fails", () => {
    const profile = createInitialTargetProfile();
    profile.requestTemplate = '{"model":"gpt-4o-mini"}';

    expect(
      canProceedFromStep(
        2,
        baseDraft({
          targetProfile: profile,
        }),
      ),
    ).toBe(false);
  });

  it("still requires step 3 verification before proceeding", () => {
    expect(canProceedFromStep(3, baseDraft({ target: { id: "t1" } as WizardDraft["target"] }))).toBe(
      false,
    );
  });
});

describe("canNavigateToStep", () => {
  it("blocks step 6 while a submitted scan is still running", () => {
    const draft = baseDraft({ submittedScanId: "scan-1" });

    expect(canNavigateToStep(5, draft, { scanStatus: "running" })).toBe(true);
    expect(canNavigateToStep(6, draft, { scanStatus: "running" })).toBe(false);
    expect(canNavigateToStep(6, draft, { scanStatus: "paused" })).toBe(false);
  });

  it("allows step 6 only after the scan completes", () => {
    const draft = baseDraft({ submittedScanId: "scan-1" });

    expect(canNavigateToStep(6, draft, { scanStatus: "completed" })).toBe(true);
    expect(canNavigateToStep(6, draft, { scanStatus: "failed" })).toBe(false);
  });
});
