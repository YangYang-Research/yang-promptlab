import { describe, expect, it } from "vitest";

import {
  ATTACK_PROFILES,
  estimateRequests,
  estimateRuntimeSeconds,
  formatEstimatedRuntime,
  getCategory,
  requestsPerCategory,
  VARIANTS_PER_PAYLOAD,
} from "@/features/scans/attackProfiles";

describe("attackProfiles", () => {
  it("maps quick profile to three core categories", () => {
    const quick = ATTACK_PROFILES.find((p) => p.id === "quick");
    expect(quick?.categories).toEqual([
      "prompt_injection",
      "jailbreak",
      "system_prompt_extraction",
    ]);
  });

  it("estimates requests as payloads × variants × categories × endpoints", () => {
    const perCategory = requestsPerCategory(getCategory("prompt_injection"));
    expect(perCategory).toBe(3 * VARIANTS_PER_PAYLOAD);

    const requests = estimateRequests({
      selectedEndpointCount: 2,
      profileId: "quick",
    });
    expect(requests).toBe(perCategory * 3 * 2);
  });

  it("reduces estimates when tests are disabled", () => {
    const full = estimateRequests({
      selectedEndpointCount: 1,
      profileId: "quick",
    });
    const partial = estimateRequests({
      selectedEndpointCount: 1,
      profileId: "quick",
      disabledTestIds: new Set(["pi-direct-override", "pi-indirect-task", "pi-markdown-fence"]),
    });
    expect(partial).toBe(full - requestsPerCategory(getCategory("prompt_injection")));
  });

  it("formats runtime for sub-minute and multi-minute scans", () => {
    expect(formatEstimatedRuntime(0)).toBe("—");
    expect(formatEstimatedRuntime(45)).toBe("~45s");
    expect(formatEstimatedRuntime(125)).toBe("~2m 5s");
  });

  it("derives runtime from request count", () => {
    const seconds = estimateRuntimeSeconds({
      selectedEndpointCount: 1,
      profileId: "standard",
    });
    expect(seconds).toBeGreaterThan(0);
  });
});
