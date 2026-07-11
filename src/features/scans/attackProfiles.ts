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

export type ExecutionStrategy = "sequential" | "agentic";

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
  /** Custom mode only — preset modes use AI Runtime `profileModes`. */
  categories?: AttackCategoryId[];
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
    payloadCount: 29,
    mutatorCount: 3,
    tests: [
      { id: "pi-direct-override", name: "Direct instruction override" },
      { id: "enc-ignore-rules", name: "Encoding probe base" },
      { id: "pi-force-output", name: "Force fixed output / compliance probe" },
      { id: "pi-indirect-task", name: "Indirect task injection" },
      { id: "pi-indirect-rag", name: "Indirect injection via retrieved content" },
      { id: "pi-indirect-tool", name: "Indirect injection via tool/MCP result" },
      { id: "pi-indirect-web", name: "Indirect via HTML/JS comments" },
      { id: "pi-indirect-exif", name: "Indirect via document/EXIF metadata" },
      { id: "pi-indirect-api", name: "Indirect via malicious API JSON" },
      { id: "pi-markdown-fence", name: "Markdown fence escape" },
      { id: "pi-xml-tag", name: "XML / chat-delimiter injection" },
      { id: "pi-role-spoof", name: "Spoof system role (role:system / <<SYS>>)" },
      { id: "pi-sandwich", name: "Prompt sandwiching" },
      { id: "pi-recursive", name: "Recursive / self-prompt generation" },
      { id: "pi-context-bias", name: "Context injection via external claims" },
      { id: "pi-logical-confusion", name: "Ambiguous just-testing intent" },
      { id: "pi-identity-inject", name: "Impersonate privileged role" },
      { id: "pi-reverse-render", name: "Reverse PI (payload rendered later)" },
      { id: "pi-third-party-wrapper", name: "Inject via API wrapper prompt field" },
      { id: "pi-output-format", name: "Force alternate output format" },
      { id: "pi-cot-bypass", name: "Chain-of-thought / simulated-thought bypass" },
      { id: "ioh-html-xss", name: "Force XSS/HTML in model output" },
      { id: "ioh-sqli-payload", name: "Force SQLi string in output toward sink" },
      { id: "ioh-script-reflect", name: "Input reflection script alert" },
      { id: "ioh-code-exec-sink", name: "Output reaches code/exec sink" },
      { id: "uc-token-bomb", name: "Token / context bomb" },
      { id: "uc-nested-flood", name: "Deeply nested / complex overload" },
      { id: "uc-endless-story", name: "Never-ending generation DoS" },
      { id: "uc-retry-amplify", name: "Retry / tool-loop cost amplify" },
    ],
  },
  {
    id: "system_prompt_extraction",
    label: "System Prompt Extraction",
    description: "Attempts to leak hidden system instructions",
    payloadCount: 6,
    mutatorCount: 3,
    tests: [
      { id: "spe-repeat", name: "Repeat initialization / system text" },
      { id: "spe-translate", name: "Translate system prompt" },
      { id: "spe-json-dump", name: "JSON configuration dump" },
      { id: "spe-role-reveal", name: "Role / policy meta-reveal" },
      { id: "spe-diff-probe", name: "Incremental completion / diff leak" },
      { id: "spe-disregard-leak", name: "Disregard and provide complete prompt text" },
    ],
  },
  {
    id: "jailbreak",
    label: "Jailbreak",
    description: "Roleplay and framing to bypass safety policies",
    payloadCount: 10,
    mutatorCount: 3,
    tests: [
      { id: "jb-dan", name: "DAN / developer-mode roleplay" },
      { id: "jb-opposite", name: "Opposite-mode policy invert" },
      { id: "jb-hypothetical", name: "Hypothetical / fictional framing" },
      { id: "jb-developer-mode", name: "Fake developer / unrestricted mode" },
      { id: "jb-system-persona", name: "You are the system prompt role inject" },
      { id: "jb-encoding-obfuscation", name: "Encoded / obfuscated policy bypass" },
      { id: "jb-unicode-homoglyph", name: "Unicode / homoglyph masking" },
      { id: "jb-base64-decode", name: "Base64 decode-and-follow" },
      { id: "jb-html-entity", name: "HTML entity / unicode-escape bypass" },
      { id: "jb-rce-wrap", name: "Jailbreak wrapper around code-exec ask" },
    ],
  },
  {
    id: "rag_leakage",
    label: "RAG Leakage",
    description: "Retrieval context and source document disclosure",
    payloadCount: 5,
    mutatorCount: 3,
    tests: [
      { id: "rag-source-dump", name: "Source document dump" },
      { id: "rag-context-repeat", name: "Retrieved context repetition" },
      { id: "rag-metadata", name: "Vector metadata extraction" },
      { id: "rag-cross-collection", name: "Cross-collection retrieval" },
      { id: "rag-embedding-inversion", name: "Neighbor / embedding inversion probe" },
    ],
  },
  {
    id: "memory_poisoning",
    label: "Memory Poisoning",
    description: "Persistent false facts and preference overrides",
    payloadCount: 5,
    mutatorCount: 3,
    tests: [
      { id: "mp-persist-instruction", name: "Persistent instruction in memory" },
      { id: "mp-false-fact", name: "False fact injection" },
      { id: "mp-preference-override", name: "Preference / policy override via memory" },
      { id: "dp-rag-poison", name: "Poison document into retrieval corpus" },
      { id: "dp-session-state", name: "Corrupt session / scratchpad state" },
    ],
  },
  {
    id: "cross_user_leakage",
    label: "Cross User Leakage",
    description: "Tenant and session isolation bypass",
    payloadCount: 7,
    mutatorCount: 3,
    tests: [
      { id: "sd-secret-exfil", name: "API key / credential fishing" },
      { id: "sd-pii-extract", name: "PII extraction from context" },
      { id: "sd-internal-config", name: "Internal config / endpoint disclosure" },
      { id: "sd-output-echo", name: "Force logging / secret echo in response" },
      { id: "cul-prev-user", name: "Previous user history leak" },
      { id: "cul-user-id", name: "Arbitrary user lookup" },
      { id: "cul-tenant-bleed", name: "Tenant isolation bypass" },
    ],
  },
  {
    id: "agent_goal_hijacking",
    label: "Agent Goal Hijacking",
    description: "Planner and goal manipulation for autonomous agents",
    payloadCount: 5,
    mutatorCount: 3,
    tests: [
      { id: "agh-new-goal", name: "Goal replacement" },
      { id: "agh-planner-inject", name: "Planner / plan-step injection" },
      { id: "agh-priority-flip", name: "Priority / stop-condition inversion" },
      { id: "agh-tool-loop", name: "Force unbounded tool loop" },
      { id: "agh-delegate-abuse", name: "Sub-agent / delegate goal hijack" },
    ],
  },
  {
    id: "tool_abuse",
    label: "Tool Abuse",
    description: "Shell, SQL, and filesystem tool misuse",
    payloadCount: 11,
    mutatorCount: 3,
    tests: [
      { id: "ta-shell", name: "Shell invocation via tool" },
      { id: "ta-sql-inject", name: "SQL injection through tool args" },
      { id: "ta-path-traversal", name: "File read path traversal" },
      { id: "ta-exfil-tool", name: "Data exfil via outbound tool/URL" },
      { id: "ta-permission-bypass", name: "Call tool outside allow-list" },
      { id: "ta-arg-injection", name: "Nested / encoded tool-arg injection" },
      { id: "ta-ssrf", name: "SSRF via fetch/browse tool" },
      { id: "ta-code-interpreter-rce", name: "RCE via code interpreter" },
      { id: "ta-network-callback", name: "Network callback / curl to attacker" },
      { id: "ta-function-abuse", name: "Unsafe function generation eval sinks" },
      { id: "ta-xsrf-plugin", name: "Cross-plugin request forgery chain" },
    ],
  },
  {
    id: "mcp_abuse",
    label: "MCP Abuse",
    description: "Unauthorized MCP tool calls and resource poisoning",
    payloadCount: 7,
    mutatorCount: 3,
    tests: [
      { id: "mcp-tool-call", name: "Unauthorized MCP tool call" },
      { id: "mcp-list-tools", name: "Tool enumeration" },
      { id: "mcp-resource-poison", name: "Resource URI / content injection" },
      { id: "mcp-tool-desc-inject", name: "Malicious tool description poisoning" },
      { id: "mcp-confused-deputy", name: "Act with user privileges via MCP" },
      { id: "mcp-secret-exfil", name: "Secret exfil via tool result" },
      { id: "mcp-sampling-inject", name: "Sampling / elicit prompt injection" },
    ],
  },
];

export const ALL_ATTACK_CATEGORY_IDS: AttackCategoryId[] = ATTACK_CATALOG.map((c) => c.id);

export const ATTACK_PROFILES: AttackProfileDefinition[] = [
  {
    id: "quick",
    label: "Quick Assessment",
    description: "Minimal Yazg-selected coverage — fast execution smoke test",
  },
  {
    id: "standard",
    label: "Security Review",
    description: "Balanced Yazg-selected coverage — recommended",
  },
  {
    id: "deep",
    label: "Red Team",
    description: "Maximum Yazg-selected coverage — long runtime",
  },
  {
    id: "custom",
    label: "Custom",
    description: "Customize categories and individual tests manually",
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

export type ScanEstimateInput = {
  selectedEndpointCount: number;
  profileId: AttackProfileId;
  customCategories?: AttackCategoryId[];
  disabledTestIds?: ReadonlySet<string>;
};

export function resolveActiveCategories(input: ScanEstimateInput): AttackCategoryId[] {
  if (input.profileId === "custom") {
    return input.customCategories ?? [];
  }
  return getProfile(input.profileId).categories ?? [];
}

/** Legacy scan-history estimates — wizard Step 4 uses backend planner output instead. */
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
