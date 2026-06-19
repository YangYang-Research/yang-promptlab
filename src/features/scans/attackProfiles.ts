/** Matches `aisec-attack` `AttackCategory` serde names. */
export type AttackCategoryId =
  | "prompt_injection"
  | "system_prompt_extraction"
  | "jailbreak"
  | "rag_leakage"
  | "memory_poisoning"
  | "cross_user_leakage"
  | "agent_goal_hijacking"
  | "tool_abuse"
  | "mcp_abuse";

export type AttackProfileId = "quick" | "standard" | "deep" | "custom";

export type AttackTest = {
  id: string;
  name: string;
};

export type AttackCategoryDefinition = {
  id: AttackCategoryId;
  label: string;
  description: string;
  tests: AttackTest[];
  /** Default payloads in `default_payloads()` — all builtins use 3. */
  payloadCount: number;
  /** Mutators listed in the attack plan — all builtins use 3. */
  mutatorCount: number;
};

export type AttackProfileDefinition = {
  id: AttackProfileId;
  label: string;
  description: string;
  categories: AttackCategoryId[];
};

/** Original payload + up to 3 mutator variants (`PayloadMutator::with_defaults`). */
export const VARIANTS_PER_PAYLOAD = 4;

/** Average HTTP round-trip per payload attempt (UI estimate only). */
export const SECONDS_PER_REQUEST = 2.5;

export const ATTACK_CATALOG: AttackCategoryDefinition[] = [
  {
    id: "prompt_injection",
    label: "Prompt Injection",
    description: "Instruction override via direct and indirect injection",
    payloadCount: 3,
    mutatorCount: 3,
    tests: [
      { id: "pi-direct-override", name: "Direct instruction override" },
      { id: "pi-indirect-task", name: "Indirect task injection" },
      { id: "pi-markdown-fence", name: "Markdown fence escape" },
    ],
  },
  {
    id: "system_prompt_extraction",
    label: "System Prompt Extraction",
    description: "Attempts to leak hidden system instructions",
    payloadCount: 3,
    mutatorCount: 3,
    tests: [
      { id: "spe-repeat", name: "Repeat initialization" },
      { id: "spe-translate", name: "Translate system prompt" },
      { id: "spe-json-dump", name: "JSON configuration dump" },
    ],
  },
  {
    id: "jailbreak",
    label: "Jailbreak",
    description: "Roleplay and framing to bypass safety policies",
    payloadCount: 3,
    mutatorCount: 3,
    tests: [
      { id: "jb-dan", name: "DAN roleplay" },
      { id: "jb-opposite", name: "Opposite mode" },
      { id: "jb-hypothetical", name: "Hypothetical framing" },
    ],
  },
  {
    id: "rag_leakage",
    label: "RAG Leakage",
    description: "Retrieval context and source document disclosure",
    payloadCount: 3,
    mutatorCount: 3,
    tests: [
      { id: "rag-source-dump", name: "Source document dump" },
      { id: "rag-context-repeat", name: "Context repetition" },
      { id: "rag-metadata", name: "Vector metadata extraction" },
    ],
  },
  {
    id: "memory_poisoning",
    label: "Memory Poisoning",
    description: "Persistent false facts and preference overrides",
    payloadCount: 3,
    mutatorCount: 3,
    tests: [
      { id: "mp-persist-instruction", name: "Persistent instruction" },
      { id: "mp-false-fact", name: "False fact injection" },
      { id: "mp-preference-override", name: "Preference override" },
    ],
  },
  {
    id: "cross_user_leakage",
    label: "Cross User Leakage",
    description: "Tenant and session isolation bypass",
    payloadCount: 3,
    mutatorCount: 3,
    tests: [
      { id: "cul-prev-user", name: "Previous user history" },
      { id: "cul-user-id", name: "Arbitrary user lookup" },
      { id: "cul-tenant-bleed", name: "Tenant isolation bypass" },
    ],
  },
  {
    id: "agent_goal_hijacking",
    label: "Agent Goal Hijacking",
    description: "Planner and goal manipulation for autonomous agents",
    payloadCount: 3,
    mutatorCount: 3,
    tests: [
      { id: "agh-new-goal", name: "Goal replacement" },
      { id: "agh-planner-inject", name: "Planner injection" },
      { id: "agh-priority-flip", name: "Priority inversion" },
    ],
  },
  {
    id: "tool_abuse",
    label: "Tool Abuse",
    description: "Shell, SQL, and filesystem tool misuse",
    payloadCount: 3,
    mutatorCount: 3,
    tests: [
      { id: "ta-shell", name: "Shell invocation" },
      { id: "ta-sql-inject", name: "SQL tool injection" },
      { id: "ta-path-traversal", name: "File read traversal" },
    ],
  },
  {
    id: "mcp_abuse",
    label: "MCP Abuse",
    description: "Unauthorized MCP tool calls and resource poisoning",
    payloadCount: 3,
    mutatorCount: 3,
    tests: [
      { id: "mcp-tool-call", name: "Unauthorized MCP tool call" },
      { id: "mcp-resource-poison", name: "Resource URI injection" },
      { id: "mcp-list-tools", name: "Tool enumeration" },
    ],
  },
];

export const ALL_ATTACK_CATEGORY_IDS: AttackCategoryId[] = ATTACK_CATALOG.map((c) => c.id);

export const ATTACK_PROFILES: AttackProfileDefinition[] = [
  {
    id: "quick",
    label: "Quick",
    description: "High-signal smoke test — injection, jailbreak, and system prompt extraction",
    categories: ["prompt_injection", "jailbreak", "system_prompt_extraction"],
  },
  {
    id: "standard",
    label: "Standard",
    description: "Balanced OWASP-aligned coverage for typical LLM API assessments",
    categories: [
      "prompt_injection",
      "jailbreak",
      "system_prompt_extraction",
      "rag_leakage",
      "tool_abuse",
      "cross_user_leakage",
    ],
  },
  {
    id: "deep",
    label: "Deep",
    description: "Full engine catalog — all nine attack categories",
    categories: ALL_ATTACK_CATEGORY_IDS,
  },
  {
    id: "custom",
    label: "Custom",
    description: "Pick categories and individual tests manually",
    categories: ALL_ATTACK_CATEGORY_IDS,
  },
];

export function getCategory(id: AttackCategoryId): AttackCategoryDefinition {
  const category = ATTACK_CATALOG.find((c) => c.id === id);
  if (!category) throw new Error(`Unknown attack category: ${id}`);
  return category;
}

export function getProfile(id: AttackProfileId): AttackProfileDefinition {
  const profile = ATTACK_PROFILES.find((p) => p.id === id);
  if (!profile) throw new Error(`Unknown attack profile: ${id}`);
  return profile;
}

export function requestsPerCategory(category: AttackCategoryDefinition): number {
  return category.payloadCount * VARIANTS_PER_PAYLOAD;
}

export function enabledTestsForCategory(
  category: AttackCategoryDefinition,
  disabledTestIds: ReadonlySet<string>,
): AttackTest[] {
  return category.tests.filter((test) => !disabledTestIds.has(test.id));
}

export function requestsForCategorySelection(
  category: AttackCategoryDefinition,
  disabledTestIds: ReadonlySet<string>,
): number {
  const enabled = enabledTestsForCategory(category, disabledTestIds);
  if (enabled.length === 0) return 0;
  const ratio = enabled.length / category.tests.length;
  return Math.round(requestsPerCategory(category) * ratio);
}

export type AttackPlanConfig = {
  profileId: AttackProfileId;
  customCategories: AttackCategoryId[];
  disabledTests: string[];
  categories: AttackCategoryId[];
  generatorMode: GeneratorMode;
  agentMode: boolean;
  maxAgentAttempts: number;
};

export type GeneratorMode = "static_pack" | "template_mutation" | "local_llm";

export type ScanEstimateInput = {
  selectedEndpointCount: number;
  profileId: AttackProfileId;
  /** Used when profileId is `custom`; ignored for presets. */
  customCategories?: AttackCategoryId[];
  disabledTestIds?: ReadonlySet<string>;
};

export function resolveActiveCategories(input: ScanEstimateInput): AttackCategoryId[] {
  if (input.profileId === "custom") {
    return input.customCategories ?? [];
  }
  return getProfile(input.profileId).categories;
}

export function estimateRequests(input: ScanEstimateInput): number {
  const { selectedEndpointCount } = input;
  if (selectedEndpointCount <= 0) return 0;

  const disabled = input.disabledTestIds ?? new Set<string>();
  const categories = resolveActiveCategories(input);

  let perEndpoint = 0;
  for (const id of categories) {
    perEndpoint += requestsForCategorySelection(getCategory(id), disabled);
  }
  return perEndpoint * selectedEndpointCount;
}

export function estimateRuntimeSeconds(input: ScanEstimateInput): number {
  return Math.ceil(estimateRequests(input) * SECONDS_PER_REQUEST);
}

export function formatEstimatedRuntime(totalSeconds: number): string {
  if (totalSeconds <= 0) return "—";
  if (totalSeconds < 60) return `~${totalSeconds}s`;

  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (minutes < 60) {
    return seconds > 0 ? `~${minutes}m ${seconds}s` : `~${minutes}m`;
  }

  const hours = Math.floor(minutes / 60);
  const remMinutes = minutes % 60;
  return remMinutes > 0 ? `~${hours}h ${remMinutes}m` : `~${hours}h`;
}
