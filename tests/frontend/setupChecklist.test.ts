import { describe, expect, it } from "vitest";

import { deriveSetupSteps, setupProgress } from "@/features/setup/setupSteps";

describe("deriveSetupSteps", () => {
  it("marks nothing done on a fresh install and locks later steps", () => {
    const steps = deriveSetupSteps({
      mode: "not_configured",
      initialized: false,
      localModelCount: 0,
      configuredThirdPartyCount: 0,
      selectedModelId: null,
      modelLoaded: false,
      projectCount: 0,
      scanCount: 0,
    });
    expect(setupProgress(steps).doneCount).toBe(0);
    expect(steps.map((s) => s.locked)).toEqual([false, true, true, true]);
    expect(steps.map((s) => s.to)).toEqual([
      "/runtime",
      "/models",
      "/runtime",
      "/scans/new",
    ]);
  });

  it("does not count later steps done until prior steps finish", () => {
    const steps = deriveSetupSteps({
      mode: "not_configured",
      initialized: false,
      localModelCount: 2,
      configuredThirdPartyCount: 1,
      selectedModelId: "m1",
      modelLoaded: true,
      projectCount: 1,
      scanCount: 1,
    });
    expect(steps.map((s) => s.done)).toEqual([false, false, false, false]);
    expect(steps[0]?.locked).toBe(false);
    expect(steps.slice(1).every((s) => s.locked)).toBe(true);
  });

  it("unlocks the next step only after the previous is done", () => {
    const afterMode = deriveSetupSteps({
      mode: "local",
      initialized: true,
      localModelCount: 0,
      configuredThirdPartyCount: 0,
      selectedModelId: null,
      modelLoaded: false,
      projectCount: 0,
      scanCount: 0,
    });
    expect(afterMode.map((s) => s.done)).toEqual([true, false, false, false]);
    expect(afterMode.map((s) => s.locked)).toEqual([false, false, true, true]);
  });

  it("tracks local runtime progress through load", () => {
    const steps = deriveSetupSteps({
      mode: "local",
      initialized: true,
      localModelCount: 1,
      configuredThirdPartyCount: 0,
      selectedModelId: "m1",
      modelLoaded: true,
      projectCount: 0,
      scanCount: 0,
    });
    expect(steps.filter((s) => s.done).map((s) => s.id)).toEqual([
      "runtime-mode",
      "add-model",
      "load-model",
    ]);
    expect(steps.map((s) => s.locked)).toEqual([false, false, false, false]);
    expect(setupProgress(steps).allDone).toBe(false);
  });

  it("completes when project and scan exist", () => {
    const steps = deriveSetupSteps({
      mode: "third_party",
      initialized: true,
      localModelCount: 0,
      configuredThirdPartyCount: 1,
      selectedModelId: "gpt",
      modelLoaded: false,
      projectCount: 1,
      scanCount: 1,
    });
    expect(setupProgress(steps).allDone).toBe(true);
    expect(steps.every((s) => !s.locked)).toBe(true);
  });
});
