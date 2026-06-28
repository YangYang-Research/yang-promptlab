import { describe, expect, it } from "vitest";

import { createInitialTargetProfile } from "@/features/scans/targetProfile";
import { canProceedFromStep, type WizardDraft } from "@/features/scans/wizardSteps";

function baseDraft(overrides: Partial<WizardDraft> = {}): WizardDraft {
  return {
    projectId: "project-1",
    target: null,
    targetProfile: createInitialTargetProfile(),
    targetForm: {} as WizardDraft["targetForm"],
    profileVerified: false,
    attackPlan: null,
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
