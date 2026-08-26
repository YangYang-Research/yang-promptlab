import {
  ALL_ATTACK_CATEGORY_IDS,
  type AttackCategoryId,
} from "@/features/scans/attackProfiles";
import {
  ALL_ATTACK_MUTATOR_IDS,
  loadAppEnabledMutators,
  normalizeEnabledMutators,
  QUICK_ATTACK_MUTATOR_IDS,
  setAppEnabledMutatorsCache,
  type AttackMutatorId,
} from "@/features/scans/payloadStrategy";
import {
  getMutatorSettings,
  setMutatorSettings,
  type MutatorSettingsDto,
} from "@/shared/ipc/mutators";

/** Built-in defaults — used when DB map is empty / reset.
 *  Order = priority: with low `variantsPerTest`, only the first N−1 run. */
export const DEFAULT_CATEGORY_MUTATORS: Record<AttackCategoryId, AttackMutatorId[]> = {
  prompt_injection: [
    "delimiter_injection",
    "language_pivot",
    "role_swap",
    "markdown_code_fence",
    "base64_wrap",
    "html_wrap",
    "hex_wrap",
    "token_split",
  ],
  jailbreak: [
    "role_swap",
    "language_pivot",
    "unicode_homoglyph",
    "base64_wrap",
    "html_wrap",
    "leetspeak",
    "chunk_split",
    "zero_width_dense",
    "rot13_wrap",
    "reversed_text",
  ],
  system_prompt_extraction: [
    "repeat_amplify",
    "language_pivot",
    "delimiter_injection",
    "role_swap",
    "markdown_code_fence",
    "base64_wrap",
    "hex_wrap",
  ],
  tool_abuse: [
    "json_escape",
    "html_wrap",
    "base64_wrap",
    "delimiter_injection",
    "token_split",
    "markdown_code_fence",
  ],
  mcp_abuse: [
    "json_escape",
    "html_wrap",
    "base64_wrap",
    "delimiter_injection",
    "token_split",
    "markdown_code_fence",
  ],
  rag_leakage: [
    "repeat_amplify",
    "delimiter_injection",
    "markdown_code_fence",
    "zero_width_dense",
    "token_split",
  ],
  memory_poisoning: [
    "repeat_amplify",
    "language_pivot",
    "role_swap",
    "delimiter_injection",
    "markdown_code_fence",
    "chunk_split",
  ],
  cross_user_leakage: [
    "role_swap",
    "delimiter_injection",
    "repeat_amplify",
    "markdown_code_fence",
    "zero_width_dense",
  ],
  agent_goal_hijacking: [
    "role_swap",
    "language_pivot",
    "delimiter_injection",
    "markdown_code_fence",
    "repeat_amplify",
    "token_split",
  ],
};

export type CategoryMutatorMap = Record<AttackCategoryId, AttackMutatorId[]>;

export const QUICK_MUTATOR_PRESET = QUICK_ATTACK_MUTATOR_IDS;

export function emptyCategoryMutatorMap(): CategoryMutatorMap {
  return Object.fromEntries(
    ALL_ATTACK_CATEGORY_IDS.map((id) => [id, [] as AttackMutatorId[]]),
  ) as CategoryMutatorMap;
}

export function normalizeCategoryMutatorMap(
  raw: Record<string, string[]> | undefined | null,
): CategoryMutatorMap {
  const base = emptyCategoryMutatorMap();
  if (!raw) {
    for (const id of ALL_ATTACK_CATEGORY_IDS) {
      base[id] = [...DEFAULT_CATEGORY_MUTATORS[id]];
    }
    return base;
  }
  for (const id of ALL_ATTACK_CATEGORY_IDS) {
    base[id] = normalizeEnabledMutators(raw[id] ?? []);
  }
  return base;
}

export function categoriesForMutator(
  map: CategoryMutatorMap,
  mutatorId: AttackMutatorId,
): AttackCategoryId[] {
  return ALL_ATTACK_CATEGORY_IDS.filter((category) => map[category].includes(mutatorId));
}

export function toggleMutatorCategory(
  map: CategoryMutatorMap,
  mutatorId: AttackMutatorId,
  categoryId: AttackCategoryId,
  enabled: boolean,
): CategoryMutatorMap {
  const next = { ...map, [categoryId]: [...map[categoryId]] };
  if (enabled) {
    if (!next[categoryId].includes(mutatorId)) {
      next[categoryId] = [...next[categoryId], mutatorId];
    }
  } else {
    next[categoryId] = next[categoryId].filter((id) => id !== mutatorId);
  }
  return next;
}

/** Restore one mutator's category assignments to DEFAULT_CATEGORY_MUTATORS. */
export function resetMutatorToDefaultCategories(
  map: CategoryMutatorMap,
  mutatorId: AttackMutatorId,
): CategoryMutatorMap {
  let next = map;
  for (const categoryId of ALL_ATTACK_CATEGORY_IDS) {
    const shouldBeAssigned = DEFAULT_CATEGORY_MUTATORS[categoryId].includes(mutatorId);
    const isAssigned = next[categoryId].includes(mutatorId);
    if (shouldBeAssigned !== isAssigned) {
      next = toggleMutatorCategory(next, mutatorId, categoryId, shouldBeAssigned);
    }
  }
  return next;
}

export function isMutatorAtDefaultCategories(
  map: CategoryMutatorMap,
  mutatorId: AttackMutatorId,
): boolean {
  return ALL_ATTACK_CATEGORY_IDS.every(
    (categoryId) =>
      DEFAULT_CATEGORY_MUTATORS[categoryId].includes(mutatorId) ===
      map[categoryId].includes(mutatorId),
  );
}

export type MutatorAppSettings = {
  enabledMutators: AttackMutatorId[];
  categoryMutators: CategoryMutatorMap;
};

function fromDto(dto: MutatorSettingsDto): MutatorAppSettings {
  return {
    enabledMutators: normalizeEnabledMutators(dto.enabledMutators),
    categoryMutators: normalizeCategoryMutatorMap(dto.categoryMutators),
  };
}

export function loadEnabledMutators(): AttackMutatorId[] {
  return loadAppEnabledMutators();
}

export async function hydrateMutatorSettings(): Promise<MutatorAppSettings> {
  const dto = await getMutatorSettings();
  const settings = fromDto(dto);
  setAppEnabledMutatorsCache(settings.enabledMutators);
  return settings;
}

/** @deprecated Prefer hydrateMutatorSettings */
export async function hydrateEnabledMutators(): Promise<AttackMutatorId[]> {
  const settings = await hydrateMutatorSettings();
  return settings.enabledMutators;
}

export async function persistMutatorSettings(
  settings: MutatorAppSettings,
): Promise<MutatorAppSettings> {
  const dto = await setMutatorSettings({
    enabledMutators: normalizeEnabledMutators(settings.enabledMutators),
    categoryMutators: settings.categoryMutators,
  });
  const saved = fromDto(dto);
  setAppEnabledMutatorsCache(saved.enabledMutators);
  return saved;
}

export async function persistEnabledMutators(
  ids: AttackMutatorId[],
): Promise<AttackMutatorId[]> {
  const current = await hydrateMutatorSettings();
  const saved = await persistMutatorSettings({
    ...current,
    enabledMutators: normalizeEnabledMutators(ids),
  });
  return saved.enabledMutators;
}

export { ALL_ATTACK_MUTATOR_IDS };
