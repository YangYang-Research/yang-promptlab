import { describe, expect, it } from "vitest";

import { resolveCategoriesForAdjust } from "@/features/scans/attackPlan";

describe("resolveCategoriesForAdjust", () => {
  const suggested = [
    "prompt_injection",
    "jailbreak",
    "system_prompt_extraction",
    "tool_abuse",
  ] as const;

  it("returns suggested categories for preset profiles", () => {
    expect(
      resolveCategoriesForAdjust(
        "standard",
        { customCategories: [], disabledGraphNodes: [] },
        { suggestedCategories: [...suggested], categories: [...suggested] },
      ),
    ).toEqual([...suggested]);
  });

  it("falls back to active categories when custom list is empty", () => {
    expect(
      resolveCategoriesForAdjust(
        "custom",
        { customCategories: [], disabledGraphNodes: [] },
        {
          suggestedCategories: [...suggested],
          categories: ["prompt_injection", "jailbreak"],
        },
      ),
    ).toEqual(["prompt_injection", "jailbreak"]);
  });

  it("derives custom categories from disabled graph nodes", () => {
    expect(
      resolveCategoriesForAdjust(
        "custom",
        { customCategories: [], disabledGraphNodes: ["tool_abuse"] },
        { suggestedCategories: [...suggested], categories: [] },
      ),
    ).toEqual(["prompt_injection", "jailbreak", "system_prompt_extraction"]);
  });
});
