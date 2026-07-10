import type { AttackProfileId } from "./attackProfiles";

export type PayloadGenerationStrategy = "deterministic" | "mutation" | "adaptive";
export type MutationLevel = "low" | "medium" | "high" | "extreme";

export type PayloadStrategyConfig = {
  strategy: PayloadGenerationStrategy;
  mutationLevel: MutationLevel;
  variantsPerTest: number;
  maxTotalPayloads: number;
  enableContextAwareness: boolean;
  enableConversationMemory: boolean;
  enableResponseAdaptation: boolean;
  enablePayloadDeduplication: boolean;
  enableCrossCategoryMutation: boolean;
};

export type PayloadStrategyDto = {
  strategy: string;
  mutationLevel: string;
  variantsPerTest: number;
  maxTotalPayloads: number;
  enableContextAwareness: boolean;
  enableConversationMemory: boolean;
  enableResponseAdaptation: boolean;
  enablePayloadDeduplication: boolean;
  enableCrossCategoryMutation: boolean;
};

/** Matches `PayloadStrategy::clamp` in `aisec-target-profile`. */
export const VARIANTS_PER_TEST_MIN = 1;
export const VARIANTS_PER_TEST_MAX = 20;
export const PAYLOAD_BUDGET_MIN = 1;
export const PAYLOAD_BUDGET_MAX = 50;
export const PAYLOAD_BUDGET_DEFAULT = 20;
export const PAYLOAD_BUDGET_STEP = 1;

export function clampVariantsPerTest(value: number): number {
  return Math.min(VARIANTS_PER_TEST_MAX, Math.max(VARIANTS_PER_TEST_MIN, value));
}

export function clampPayloadBudget(value: number): number {
  return Math.min(
    PAYLOAD_BUDGET_MAX,
    Math.max(PAYLOAD_BUDGET_MIN, Math.round(value)),
  );
}

export function sliderPercent(value: number, min: number, max: number): number {
  if (max <= min) return 0;
  return Math.min(100, Math.max(0, ((value - min) / (max - min)) * 100));
}

export const GENERATION_STRATEGIES: Array<{
  id: PayloadGenerationStrategy;
  label: string;
  description: string;
  tooltip: string;
}> = [
  {
    id: "deterministic",
    label: "Deterministic",
    description: "Curated templates only — repeatable, no AI generation.",
    tooltip:
      "Uses curated payload templates only. No AI generation. Repeatable across runs.",
  },
  {
    id: "mutation",
    label: "Mutation",
    description: "Template-based semantic variations.",
    tooltip: "Starts from templates and produces semantic variations via mutators.",
  },
  {
    id: "adaptive",
    label: "Adaptive",
    description: "Payloads evolve using previous responses.",
    tooltip:
      "Payloads evolve using observed responses. Best with agentic execution and reflection.",
  },
];

export const MUTATION_LEVELS: Array<{
  id: MutationLevel;
  label: string;
  description: string;
  tooltip: string;
}> = [
  {
    id: "low",
    label: "Low",
    description: "Minimal wording changes — mostly original templates.",
    tooltip: "Minimal wording changes. High template reuse.",
  },
  {
    id: "medium",
    label: "Medium",
    description: "Moderate paraphrasing and encoding variants.",
    tooltip: "Moderate paraphrasing and encoding variants.",
  },
  {
    id: "high",
    label: "High",
    description: "Aggressive mutations across templates and encodings.",
    tooltip: "Aggressive mutations across templates and encodings.",
  },
  {
    id: "extreme",
    label: "Extreme",
    description: "Maximum diversity — best for deep red team scans.",
    tooltip: "Maximum diversity — useful for red team profiles.",
  },
];

export const ADVANCED_OPTIONS: Array<{
  key: keyof Pick<
    PayloadStrategyConfig,
    | "enableContextAwareness"
    | "enableConversationMemory"
    | "enableResponseAdaptation"
    | "enablePayloadDeduplication"
    | "enableCrossCategoryMutation"
  >;
  label: string;
  tooltip: string;
}> = [
  {
    key: "enableContextAwareness",
    label: "Context-aware payloads",
    tooltip:
      "Payload generator may use target profile, conversation history, capability graph, and prior responses.",
  },
  {
    key: "enableConversationMemory",
    label: "Conversation memory",
    tooltip:
      "Reference prior prompts, responses, and conversation state — useful for multi-turn jailbreak and memory leakage.",
  },
  {
    key: "enableResponseAdaptation",
    label: "Adaptive payload evolution",
    tooltip:
      "Judge → planner → generator may adapt later payloads based on refusals, defenses, and tool behavior.",
  },
  {
    key: "enablePayloadDeduplication",
    label: "Remove duplicate payloads",
    tooltip: "Strip duplicate prompts and semantic duplicates before execution.",
  },
  {
    key: "enableCrossCategoryMutation",
    label: "Cross-category evolution",
    tooltip:
      "Blend techniques across categories (e.g. prompt injection × tool abuse) into hybrid probes.",
  },
];

function asStrategy(value: string): PayloadGenerationStrategy {
  if (value === "deterministic" || value === "adaptive") return value;
  return "mutation";
}

function asMutationLevel(value: string): MutationLevel {
  if (value === "low" || value === "high" || value === "extreme") return value;
  return "medium";
}

export function payloadStrategyFromDto(dto: PayloadStrategyDto): PayloadStrategyConfig {
  return {
    strategy: asStrategy(dto.strategy),
    mutationLevel: asMutationLevel(dto.mutationLevel),
    variantsPerTest: dto.variantsPerTest,
    maxTotalPayloads: dto.maxTotalPayloads,
    enableContextAwareness: dto.enableContextAwareness,
    enableConversationMemory: dto.enableConversationMemory,
    enableResponseAdaptation: dto.enableResponseAdaptation,
    enablePayloadDeduplication: dto.enablePayloadDeduplication,
    enableCrossCategoryMutation: dto.enableCrossCategoryMutation,
  };
}

export function payloadStrategyToDto(strategy: PayloadStrategyConfig): PayloadStrategyDto {
  return {
    strategy: strategy.strategy,
    mutationLevel: strategy.mutationLevel,
    variantsPerTest: strategy.variantsPerTest,
    maxTotalPayloads: strategy.maxTotalPayloads,
    enableContextAwareness: strategy.enableContextAwareness,
    enableConversationMemory: strategy.enableConversationMemory,
    enableResponseAdaptation: strategy.enableResponseAdaptation,
    enablePayloadDeduplication: strategy.enablePayloadDeduplication,
    enableCrossCategoryMutation: strategy.enableCrossCategoryMutation,
  };
}

export function formatPayloadGenerationStrategy(strategy: PayloadStrategyConfig): string {
  return GENERATION_STRATEGIES.find((item) => item.id === strategy.strategy)?.label ?? strategy.strategy;
}

export function formatPayloadStrategySummary(strategy: PayloadStrategyConfig): string {
  const gen = GENERATION_STRATEGIES.find((item) => item.id === strategy.strategy)?.label ?? strategy.strategy;
  const mut =
    MUTATION_LEVELS.find((item) => item.id === strategy.mutationLevel)?.label ?? strategy.mutationLevel;
  return `${gen} · ${mut} · ${strategy.variantsPerTest} variants/test · budget ${strategy.maxTotalPayloads}`;
}

export function payloadStrategyMatchesRecommendation(
  current: PayloadStrategyConfig,
  recommended: PayloadStrategyConfig,
): boolean {
  return JSON.stringify(current) === JSON.stringify(recommended);
}

export function profileAppliesPayloadPreset(profileId: AttackProfileId): boolean {
  return profileId !== "custom";
}

/** Mirrors `payload_strategy_for_attack_profile` in `aisec-target-profile`. */
export function payloadStrategyForAttackProfile(
  profileId: AttackProfileId,
  recommended: PayloadStrategyConfig,
): PayloadStrategyConfig {
  switch (profileId) {
    case "quick":
      return {
        strategy: "deterministic",
        mutationLevel: "low",
        variantsPerTest: 2,
        maxTotalPayloads: 10,
        enableContextAwareness: false,
        enableConversationMemory: false,
        enableResponseAdaptation: false,
        enablePayloadDeduplication: true,
        enableCrossCategoryMutation: false,
      };
    case "deep":
      return {
        strategy: "adaptive",
        mutationLevel: "extreme",
        variantsPerTest: 10,
        maxTotalPayloads: PAYLOAD_BUDGET_MAX,
        enableContextAwareness: true,
        enableConversationMemory: true,
        enableResponseAdaptation: true,
        enablePayloadDeduplication: true,
        enableCrossCategoryMutation: true,
      };
    case "custom":
      return { ...recommended };
    default:
      return {
        strategy: "mutation",
        mutationLevel: "medium",
        variantsPerTest: 5,
        maxTotalPayloads: recommended.maxTotalPayloads || PAYLOAD_BUDGET_DEFAULT,
        enableContextAwareness: recommended.enableContextAwareness,
        enableConversationMemory: recommended.enableConversationMemory,
        enableResponseAdaptation: false,
        enablePayloadDeduplication: true,
        enableCrossCategoryMutation: false,
      };
  }
}
