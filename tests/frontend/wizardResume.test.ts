import { describe, expect, it } from "vitest";

import { attackPlanFromExecutionPlaybook } from "@/features/scans/attackPlan";
import {
  sessionReadyForSubmitStep,
  sessionReadyForWizardEntry,
} from "@/features/scans/wizardResume";
import { createInitialSession } from "@/features/scans/wizardState";
import type { Target } from "@/shared/types";

describe("wizardResume", () => {
  it("accepts a live scan resume when submittedScanId is set without local verification flag", () => {
    const session = {
      ...createInitialSession("project-1"),
      draftScanId: "scan-1",
      submittedScanId: "scan-1",
      attackPlan: attackPlanFromExecutionPlaybook({
        profile: "standard",
        categories: ["prompt_injection", "jailbreak"],
        disabled_tests: [],
        agent_mode: false,
      }),
    };
    const target = { id: "target-1" } as Target;

    expect(sessionReadyForSubmitStep(session, target)).toBe(true);
  });

  it("does not treat submittedScanId as ready for step 4 retry without verification", () => {
    const session = {
      ...createInitialSession("project-1"),
      draftScanId: "scan-1",
      submittedScanId: "scan-1",
      attackPlan: attackPlanFromExecutionPlaybook({
        profile: "standard",
        categories: ["prompt_injection"],
        disabled_tests: [],
      }),
    };
    const target = { id: "target-1" } as Target;

    expect(sessionReadyForSubmitStep(session, target)).toBe(true);
    expect(sessionReadyForWizardEntry(session, target, 4)).toBe(false);
  });

  it("allows step 4 entry when verification is present", () => {
    const session = {
      ...createInitialSession("project-1"),
      draftScanId: "scan-1",
      savedTargetId: "target-1",
      targetProfile: {
        ...createInitialSession("project-1").targetProfile,
        verification: {
          verified: true,
          status: "success",
          verifiedAt: "2026-01-01T00:00:00Z",
          statusCode: 200,
          responseTimeMs: 120,
          errorMessage: null,
        },
      },
    };
    const target = { id: "target-1" } as Target;

    expect(sessionReadyForWizardEntry(session, target, 4)).toBe(true);
  });

  it("rebuilds attack plan categories from execution playbook", () => {
    const plan = attackPlanFromExecutionPlaybook({
      profile: "standard",
      categories: ["prompt_injection", "jailbreak"],
      disabled_tests: ["pi-direct-override"],
      agent_mode: true,
      max_agent_attempts: 7,
      payload_strategy: {
        strategy: "mutation",
        mutationLevel: "medium",
        variantsPerTest: 4,
        maxTotalPayloads: 12,
        enableContextAwareness: false,
        enableConversationMemory: false,
        enableResponseAdaptation: false,
        enablePayloadDeduplication: true,
        enableCrossCategoryMutation: false,
      },
    });

    expect(plan?.categories).toEqual(["prompt_injection", "jailbreak"]);
    expect(plan?.executionStrategy).toBe("agentic");
    expect(plan?.maxAttempts).toBe(7);
    expect(plan?.estimatedRequests).toBeGreaterThan(0);
  });
});
