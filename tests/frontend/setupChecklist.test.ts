import { describe, expect, it } from "vitest";

import { deriveSetupSteps, setupProgress } from "@/features/setup/setupSteps";

describe("deriveSetupSteps", () => {
  it("marks nothing done on a fresh install and locks later steps", () => {
    const steps = deriveSetupSteps({
      mode: "third_party",
      initialized: true,
      localModelCount: 0,
      configuredThirdPartyCount: 0,
      selectedModelId: null,
      modelLoaded: false,
      projectCount: 0,
      scanCount: 0,
    });
    expect(setupProgress(steps).doneCount).toBe(0);
    expect(steps.map((s) => s.id)).toEqual(["add-model", "load-model", "first-project-scan"]);
    expect(steps.map((s) => s.locked)).toEqual([false, true, true]);
    expect(steps.map((s) => s.to)).toEqual(["/models", "/runtime", "/projects"]);
    expect(steps[2]?.linkState).toEqual({ openNewProject: true });
  });

  it("does not count later steps done until prior steps finish", () => {
    const steps = deriveSetupSteps({
      mode: "third_party",
      initialized: true,
      localModelCount: 0,
      configuredThirdPartyCount: 1,
      selectedModelId: "m1",
      modelLoaded: false,
      projectCount: 1,
      scanCount: 1,
    });
    // add-model done; load-model done; first-project-scan done
    expect(steps.map((s) => s.done)).toEqual([true, true, true]);
    expect(steps.every((s) => !s.locked)).toBe(true);
  });

  it("unlocks choose-model only after a remote model is registered", () => {
    const afterAdd = deriveSetupSteps({
      mode: "third_party",
      initialized: true,
      localModelCount: 0,
      configuredThirdPartyCount: 1,
      selectedModelId: null,
      modelLoaded: false,
      projectCount: 0,
      scanCount: 0,
    });
    expect(afterAdd.map((s) => s.done)).toEqual([true, false, false]);
    expect(afterAdd.map((s) => s.locked)).toEqual([false, false, true]);
  });

  it("routes final step to scan wizard when a project already exists", () => {
    const steps = deriveSetupSteps({
      mode: "third_party",
      initialized: true,
      localModelCount: 0,
      configuredThirdPartyCount: 1,
      selectedModelId: "m1",
      modelLoaded: false,
      projectCount: 1,
      scanCount: 0,
    });
    expect(steps[2]?.to).toBe("/scans/new");
    expect(steps[2]?.linkState).toBeUndefined();
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
    expect(steps[2]?.to).toBe("/scans/new");
  });
});
