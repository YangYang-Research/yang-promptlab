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
  tooltip: string;
}> = [
  {
    id: "low",
    label: "Low",
    tooltip: "Minimal wording changes. High template reuse.",
  },
  {
    id: "medium",
    label: "Medium",
    tooltip: "Moderate paraphrasing and encoding variants.",
  },
  {
    id: "high",
    label: "High",
    tooltip: "Aggressive mutations across templates and encodings.",
  },
  {
    id: "extreme",
    label: "Extreme",
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
