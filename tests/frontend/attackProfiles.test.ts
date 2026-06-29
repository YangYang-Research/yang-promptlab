import { describe, expect, it } from "vitest";

import { formatEstimatedRuntime } from "@/features/scans/attackPlan";
import {
  ATTACK_PROFILES,
  getCategory,
  requestsPerCategory,
  VARIANTS_PER_PAYLOAD,
} from "@/features/scans/attackProfiles";

describe("attackProfiles", () => {
  it("exposes preset attack mode labels without fixed categories", () => {
    const quick = ATTACK_PROFILES.find((p) => p.id === "quick");
    expect(quick?.label).toBe("Quick Assessment");
    expect(quick?.categories).toBeUndefined();

    const standard = ATTACK_PROFILES.find((p) => p.id === "standard");
    expect(standard?.label).toBe("Security Review");
    expect(standard?.categories).toBeUndefined();

    const deep = ATTACK_PROFILES.find((p) => p.id === "deep");
    expect(deep?.label).toBe("Red Team");
    expect(deep?.categories).toBeUndefined();
  });

  it("keeps manual categories only on custom mode", () => {
    const custom = ATTACK_PROFILES.find((p) => p.id === "custom");
    expect(custom?.categories?.length).toBeGreaterThan(0);
  });

  it("computes per-category payload variant count", () => {
    const perCategory = requestsPerCategory(getCategory("prompt_injection"));
    expect(perCategory).toBe(3 * VARIANTS_PER_PAYLOAD);
  });

  it("formats runtime for sub-minute and multi-minute scans", () => {
    expect(formatEstimatedRuntime(0)).toBe("—");
    expect(formatEstimatedRuntime(45)).toBe("~45s");
    expect(formatEstimatedRuntime(125)).toBe("~2m 5s");
  });
});
