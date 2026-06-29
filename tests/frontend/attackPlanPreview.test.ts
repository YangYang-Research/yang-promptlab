import { describe, expect, it } from "vitest";

import {
  extractPlannerEndpoint,
  formatExecutionStrategySummary,
  planCustomizationKey,
  plannerSourceFromPlan,
  previewPlanForProfile,
  recomputePlanPreview,
  resolvePlannerSummaryBadge,
  type AttackPlanConfig,
} from "@/features/scans/attackPlan";
import { createInitialAttackPlanUi } from "@/features/scans/wizardState";

function samplePayload() {
  return {
    strategy: "mutation" as const,
    mutationLevel: "medium" as const,
    variantsPerTest: 5,
    maxTotalPayloads: 20,
    enableContextAwareness: false,
    enableConversationMemory: false,
    enableResponseAdaptation: false,
    enablePayloadDeduplication: true,
    enableCrossCategoryMutation: false,
  };
}

function samplePlan(): AttackPlanConfig {
  const payload = samplePayload();
  const profileModes = [
    {
      profileId: "quick" as const,
      categories: [
        "prompt_injection",
        "jailbreak",
        "system_prompt_extraction",
      ] as const,
      executionStrategy: "sequential" as const,
      maxAttempts: 3,
      reflectionEnabled: false,
      adaptivePlanning: false,
      payloadStrategy: {
        ...payload,
        strategy: "deterministic" as const,
        mutationLevel: "low" as const,
        variantsPerTest: 2,
        maxTotalPayloads: 10,
      },
    },
    {
      profileId: "standard" as const,
      categories: [
        "prompt_injection",
        "jailbreak",
        "system_prompt_extraction",
        "tool_abuse",
      ] as const,
      executionStrategy: "sequential" as const,
      maxAttempts: 5,
      reflectionEnabled: false,
      adaptivePlanning: false,
      payloadStrategy: payload,
    },
    {
      profileId: "deep" as const,
      categories: [
        "prompt_injection",
        "jailbreak",
        "system_prompt_extraction",
        "tool_abuse",
      ] as const,
      executionStrategy: "agentic" as const,
      maxAttempts: 5,
      reflectionEnabled: true,
      adaptivePlanning: true,
      payloadStrategy: {
        ...payload,
        strategy: "adaptive" as const,
        mutationLevel: "extreme" as const,
        variantsPerTest: 10,
        maxTotalPayloads: 100,
      },
    },
  ];

  return {
    profileId: "standard",
    recommendedProfileId: "standard",
    suggestedCategories: [
      "prompt_injection",
      "jailbreak",
      "system_prompt_extraction",
      "tool_abuse",
    ],
    profileModes: profileModes.map((mode) => ({
      ...mode,
      categories: [...mode.categories],
    })),
    customCategories: [
      "prompt_injection",
      "jailbreak",
      "system_prompt_extraction",
      "tool_abuse",
    ],
    categories: [
      "prompt_injection",
      "jailbreak",
      "system_prompt_extraction",
      "tool_abuse",
    ],
    disabledTests: [],
    disabledGraphNodes: [],
    capabilityGraph: [],
    attackGraph: [
      {
        category: "prompt_injection",
        priority: 1,
        risk: 90,
        confidence: 0.9,
        dependencies: [],
        enabled: true,
      },
      {
        category: "tool_abuse",
        priority: 2,
        risk: 90,
        confidence: 0.85,
        dependencies: ["prompt_injection"],
        enabled: true,
      },
    ],
    executionStrategy: "sequential",
    maxAttempts: 5,
    reflectionEnabled: false,
    adaptivePlanning: false,
    rationales: [],
    confidence: 0.85,
    summary: "Plan for https://api.example.com/v1/chat",
    riskScore: 50,
    riskLevel: "medium",
    estimatedRequests: 60,
    estimatedRuntimeSeconds: 150,
    estimatedTokens: 28800,
    coverageScore: 0.44,
    riskCoverage: 1,
    totalTestcases: 12,
    payloadStrategy: payload,
    recommendedPayloadStrategy: payload,
  };
}

describe("attack plan preview", () => {
  it("extracts full https endpoint from planner summary", () => {
    const url = "https://api.yyng.icu/ycre/v1/code-review/github/completions";
    expect(extractPlannerEndpoint(`Plan for ${url}`)).toBe(url);
  });

  it("reduces categories and estimates when switching to quick profile", () => {
    const preview = previewPlanForProfile(samplePlan(), "quick", []);
    expect(preview.categories).toEqual([
      "prompt_injection",
      "jailbreak",
      "system_prompt_extraction",
    ]);
    expect(preview.estimatedRequests).toBeLessThan(samplePlan().estimatedRequests);
    expect(preview.categories).toHaveLength(3);
    expect(preview.attackGraph.find((node) => node.category === "tool_abuse")?.enabled).toBe(
      false,
    );
  });

  it("updates summary when execution strategy changes", () => {
    const preview = recomputePlanPreview({
      ...samplePlan(),
      executionStrategy: "agentic",
      maxAttempts: 3,
    });
    expect(preview.estimatedRequests).toBe(samplePlan().estimatedRequests * 3);
    expect(formatExecutionStrategySummary(preview)).toBe("Agentic");
  });

  it("ignores stale attack plan ui state", () => {
    const ui = {
      ...createInitialAttackPlanUi(),
      disabledGraphNodes: ["tool_abuse"],
    };
    expect(ui.disabledGraphNodes).toEqual(["tool_abuse"]);
    const preview = previewPlanForProfile(samplePlan(), "standard", []);
    expect(preview.categories).toContain("tool_abuse");
  });

  it("labels AI plans and detects customization", () => {
    const plan = {
      ...samplePlan(),
      rationales: [
        {
          category: "prompt_injection" as const,
          reason: "High exposure",
          priority: 1,
          source: "ai_runtime",
        },
      ],
    };
    const baseline = planCustomizationKey(plan);

    expect(plannerSourceFromPlan(plan)).toBe("ai_runtime");
    expect(
      resolvePlannerSummaryBadge(plan, {
        plannerSource: "ai_runtime",
        suggestedPlanKey: baseline,
      }),
    ).toEqual({ label: "AI suggested", variant: "info" });

    const customized = { ...plan, executionStrategy: "agentic" as const };
    expect(
      resolvePlannerSummaryBadge(customized, {
        plannerSource: "ai_runtime",
        suggestedPlanKey: baseline,
      }),
    ).toEqual({ label: "Customized", variant: "warning" });
  });
});
