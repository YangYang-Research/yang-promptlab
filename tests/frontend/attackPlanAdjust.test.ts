import { describe, expect, it } from "vitest";

import { resolveCategoriesForAdjust } from "@/features/scans/attackPlan";

describe("resolveCategoriesForAdjust", () => {
  const suggested = [
    "prompt_injection",
    "jailbreak",
    "system_prompt_extraction",
    "tool_abuse",
  ] as const;

  const profileModes = [
    {
      profileId: "standard" as const,
      categories: [...suggested],
      executionStrategy: "sequential" as const,
      maxAttempts: 5,
      reflectionEnabled: false,
      adaptivePlanning: false,
      payloadStrategy: {
        strategy: "mutation" as const,
        mutationLevel: "medium" as const,
        variantsPerTest: 5,
        maxTotalPayloads: 20,
        enableContextAwareness: false,
        enableConversationMemory: false,
        enableResponseAdaptation: false,
        enablePayloadDeduplication: true,
        enableCrossCategoryMutation: false,
        enabledMutators: [],
      },
    },
    {
      profileId: "quick" as const,
      categories: ["prompt_injection", "jailbreak", "system_prompt_extraction"] as const,
      executionStrategy: "sequential" as const,
      maxAttempts: 3,
      reflectionEnabled: false,
      adaptivePlanning: false,
      payloadStrategy: {
        strategy: "deterministic" as const,
        mutationLevel: "low" as const,
        variantsPerTest: 2,
        maxTotalPayloads: 10,
        enableContextAwareness: false,
        enableConversationMemory: false,
        enableResponseAdaptation: false,
        enablePayloadDeduplication: true,
        enableCrossCategoryMutation: false,
        enabledMutators: [],
      },
    },
  ];

  it("returns AI mode categories for preset profiles", () => {
    expect(
      resolveCategoriesForAdjust(
        "standard",
        { customCategories: [], disabledGraphNodes: [] },
        {
          suggestedCategories: [...suggested],
          categories: [...suggested],
          profileModes: profileModes.map((mode) => ({
            ...mode,
            categories: [...mode.categories],
            disabledTests: [],
          })),
        },
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
          profileModes: [],
        },
      ),
    ).toEqual(["prompt_injection", "jailbreak"]);
  });

  it("derives custom categories from disabled graph nodes", () => {
    expect(
      resolveCategoriesForAdjust(
        "custom",
        { customCategories: [], disabledGraphNodes: ["tool_abuse"] },
        {
          suggestedCategories: [...suggested],
          categories: [],
          profileModes: [],
        },
      ),
    ).toEqual(["prompt_injection", "jailbreak", "system_prompt_extraction"]);
  });
});
