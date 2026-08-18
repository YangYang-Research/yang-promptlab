import { describe, expect, it } from "vitest";

import {
  extractPlannerEndpoint,
  formatExecutionStrategySummary,
  plannerSourceFromPlan,
  previewPlanForProfile,
  planUiForCustomFromCategories,
  previewPlanForCustomTransition,
  recomputePlanPreview,
  resolveActivePlannerRationales,
  resolvePlannerSummaryBadge,
  syncAttackPlanUiAfterAdjust,
  type AttackPlanConfig,
} from "@/features/scans/attackPlan";
import { getCategory } from "@/features/scans/attackProfiles";
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
    enabledMutators: [] as import("@/features/scans/payloadStrategy").AttackMutatorId[],
  };
}

function samplePlan(): AttackPlanConfig {
  const payload = samplePayload();
  const profileModes = [
    {
      profileId: "quick" as const,
      description: "Quick smoke for this target.",
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
      disabledTests: [] as string[],
    },
    {
      profileId: "standard" as const,
      description: "Balanced review for this target.",
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
      disabledTests: [] as string[],
    },
    {
      profileId: "deep" as const,
      description: "Deep agentic coverage for this target.",
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
        maxTotalPayloads: 50,
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
      disabledTests: [],
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
    rationales: [
      {
        category: "prompt_injection" as const,
        reason: "AI endpoint (generic_http) — test prompt injection",
        priority: 1,
        source: "target_profile",
      },
      {
        category: "jailbreak" as const,
        reason: "LLM deployment — test safety guardrail bypass",
        priority: 2,
        source: "target_profile",
      },
      {
        category: "system_prompt_extraction" as const,
        reason: "Model API exposed — probe hidden instructions",
        priority: 3,
        source: "target_profile",
      },
      {
        category: "tool_abuse" as const,
        reason: "Tool-capable endpoint — test function abuse",
        priority: 4,
        source: "target_profile",
      },
    ],
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
    plannerSource: "ai_runtime" as const,
  };
}

describe("attack plan preview", () => {
  it("extracts full https endpoint from planner summary", () => {
    const url = "https://api.yyng.icu/ycre/v1/code-review/github/completions";
    expect(extractPlannerEndpoint(`Plan for ${url}`)).toBe(url);
  });

  it("reduces categories and estimates when switching to quick profile", () => {
    const full = recomputePlanPreview(samplePlan());
    const preview = previewPlanForProfile(samplePlan(), "quick", []);
    expect(preview.categories).toEqual([
      "prompt_injection",
      "jailbreak",
      "system_prompt_extraction",
    ]);
    expect(preview.estimatedRequests).toBeLessThan(full.estimatedRequests);
    expect(preview.categories).toHaveLength(3);
    expect(preview.attackGraph.find((node) => node.category === "tool_abuse")?.enabled).toBe(
      false,
    );
  });

  it("keeps preset categories when switching to custom and exposes the rest as disabled", () => {
    const quick = previewPlanForProfile(samplePlan(), "quick", []);
    const custom = previewPlanForCustomTransition(quick, quick.categories, [], "quick");
    const ui = planUiForCustomFromCategories(quick.categories);

    expect(custom.profileId).toBe("custom");
    expect(custom.categories).toEqual(quick.categories);
    expect(custom.customCategories).toEqual(quick.categories);
    expect(ui.customCategories).toEqual(quick.categories);
    expect(ui.disabledGraphNodes.length).toBeGreaterThan(0);
    expect(ui.disabledGraphNodes).not.toContain("prompt_injection");
    expect(custom.disabledGraphNodes).toEqual(ui.disabledGraphNodes);
    expect(custom.payloadStrategy.variantsPerTest).toBe(2);
    expect(custom.executionStrategy).toBe("sequential");
  });

  it("repairs incomplete disabledGraphNodes after custom adjust sync", () => {
    const quick = previewPlanForProfile(samplePlan(), "quick", []);
    const custom = previewPlanForCustomTransition(quick, quick.categories, [], "quick");
    // Simulate DTO round-trip that only disables nodes present in attackGraph.
    const corrupted = {
      ...custom,
      disabledGraphNodes: custom.attackGraph
        .filter((node) => !node.enabled)
        .map((node) => node.category),
    };
    expect(corrupted.disabledGraphNodes.length).toBeLessThan(
      planUiForCustomFromCategories(quick.categories).disabledGraphNodes.length,
    );

    const synced = syncAttackPlanUiAfterAdjust(corrupted, createInitialAttackPlanUi());
    expect(synced.customCategories).toEqual(quick.categories);
    expect(synced.disabledGraphNodes).toEqual(
      planUiForCustomFromCategories(quick.categories).disabledGraphNodes,
    );
    expect(synced.disabledGraphNodes).toContain("mcp_abuse");
    expect(synced.disabledGraphNodes).toContain("rag_leakage");
  });

  it("inherits execution and payload strategy from the source preset when entering custom", () => {
    const deep = previewPlanForProfile(samplePlan(), "deep", []);
    const custom = previewPlanForCustomTransition(deep, deep.categories, [], "deep");

    expect(custom.executionStrategy).toBe("agentic");
    expect(custom.reflectionEnabled).toBe(true);
    expect(custom.adaptivePlanning).toBe(true);
    expect(custom.payloadStrategy.strategy).toBe("adaptive");
    expect(custom.payloadStrategy.mutationLevel).toBe("extreme");
    expect(custom.payloadStrategy.variantsPerTest).toBe(10);
  });

  it("updates summary when execution strategy changes", () => {
    const base = recomputePlanPreview(samplePlan());
    const preview = recomputePlanPreview({
      ...samplePlan(),
      executionStrategy: "agentic",
      maxAttempts: 3,
    });
    expect(preview.estimatedRequests).toBe(base.estimatedRequests * 3);
    expect(formatExecutionStrategySummary(preview)).toBe("Agentic · Reflection · 3 attempts");
  });

  it("scales request estimate with maximum payload budget", () => {
    const base = recomputePlanPreview({
      ...samplePlan(),
      payloadStrategy: {
        ...samplePlan().payloadStrategy,
        maxTotalPayloads: 10,
      },
    });
    const doubled = recomputePlanPreview({
      ...samplePlan(),
      payloadStrategy: {
        ...samplePlan().payloadStrategy,
        maxTotalPayloads: 20,
      },
    });
    expect(doubled.estimatedRequests).toBe(base.estimatedRequests * 2);
  });

  it("estimates requests as enabledTests × variants × payloads per testcase per category", () => {
    const categories = [
      "prompt_injection",
      "jailbreak",
      "system_prompt_extraction",
    ] as const;
    const enabledTests = categories.reduce(
      (sum, id) => sum + getCategory(id).tests.length,
      0,
    );
    const preview = recomputePlanPreview({
      ...samplePlan(),
      categories: [...categories],
      disabledTests: [],
      payloadStrategy: {
        ...samplePlan().payloadStrategy,
        variantsPerTest: 2,
        maxTotalPayloads: 10,
      },
    });
    expect(preview.totalTestcases).toBe(enabledTests);
    expect(preview.estimatedRequests).toBe(enabledTests * 2 * 10);
    expect(preview.estimatedRuntimeSeconds).toBe(Math.ceil(enabledTests * 2 * 10 * 2.5));
    expect(preview.estimatedTokens).toBe(enabledTests * 2 * 10 * 480);
  });

  it("counts active tests from catalog minus disabled technique ids", () => {
    const category = getCategory("prompt_injection");
    const keep = category.tests.slice(0, 2).map((test) => test.id);
    const disabled = category.tests.slice(2).map((test) => test.id);
    const preview = recomputePlanPreview({
      ...samplePlan(),
      categories: ["prompt_injection"],
      disabledTests: disabled,
      payloadStrategy: {
        ...samplePlan().payloadStrategy,
        variantsPerTest: 1,
        maxTotalPayloads: 1,
      },
    });
    expect(preview.totalTestcases).toBe(keep.length);
    expect(preview.estimatedRequests).toBe(keep.length);
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

  it("preserves full rationale catalog when previewing a narrower profile", () => {
    const plan = samplePlan();
    const catalogLen = plan.rationales.length;
    const preview = previewPlanForProfile(plan, "standard", []);
    expect(preview.rationales).toHaveLength(catalogLen);
    expect(resolveActivePlannerRationales(preview, preview.categories).length).toBeLessThanOrEqual(
      catalogLen,
    );
    expect(resolveActivePlannerRationales(preview, preview.categories).length).toBeGreaterThan(0);
  });

  it("labels AI plans and detects customization", () => {
    const plan = {
      ...samplePlan(),
      profileId: "standard" as const,
      rationales: [
        {
          category: "prompt_injection" as const,
          reason: "High exposure",
          priority: 1,
          source: "ai_runtime",
        },
      ],
    };

    expect(plannerSourceFromPlan(plan)).toBe("ai_runtime");
    expect(
      resolvePlannerSummaryBadge(plan, {
        plannerSource: "ai_runtime",
        profileId: "standard",
      }),
    ).toEqual({ label: "AI Planned", variant: "info" });

    expect(
      resolvePlannerSummaryBadge(
        { ...plan, executionStrategy: "agentic" as const, profileId: "deep" },
        { plannerSource: "ai_runtime", profileId: "deep" },
      ),
    ).toEqual({ label: "AI Planned", variant: "info" });

    expect(
      resolvePlannerSummaryBadge(plan, {
        plannerSource: "ai_runtime",
        profileId: "custom",
      }),
    ).toEqual({ label: "Customized", variant: "warning" });
  });
});
