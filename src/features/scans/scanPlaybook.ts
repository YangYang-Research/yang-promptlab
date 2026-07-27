import {
  ATTACK_CATALOG,
  ATTACK_PROFILES,
  estimateRequests,
  estimateRuntimeSeconds,
  formatEstimatedRuntime,
  type AttackCategoryId,
  type AttackProfileId,
} from "./attackProfiles";

export type AttackScanPlaybook = {
  profile: string;
  categories: string[];
  disabledTests: string[];
  agentMode?: boolean;
  maxAgentAttempts?: number;
};

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function asStringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
}

export function parseAttackPlaybook(playbook: unknown): AttackScanPlaybook | null {
  const obj = asRecord(playbook);
  if (!obj || !obj.profile) return null;
  return {
    profile: String(obj.profile),
    categories: asStringArray(obj.categories),
    disabledTests: asStringArray(obj.disabled_tests),
    agentMode: obj.agent_mode === true,
    maxAgentAttempts:
      typeof obj.max_agent_attempts === "number" ? obj.max_agent_attempts : undefined,
  };
}

export function isAttackScanName(name: string): boolean {
  return name.startsWith("Scan (") || name.startsWith("Agent Scan (");
}

export function profileLabel(profileId: string): string {
  return ATTACK_PROFILES.find((profile) => profile.id === profileId)?.label ?? profileId;
}

export function listSelectedTests(categories: string[], disabledTests: string[]): string[] {
  return listSelectedTestsByCategory(categories, disabledTests).flatMap((group) =>
    group.tests.map((test) => `${group.label}: ${test.name}`),
  );
}

export type SelectedTestGroup = {
  categoryId: string;
  label: string;
  tests: { id: string; name: string }[];
};

/** Category groups for Attack Plan UI: all sub-tests with enabled flag + counts. */
export type PlanCategoryGroup = {
  categoryId: string;
  label: string;
  enabledCount: number;
  totalCount: number;
  tests: { id: string; name: string; enabled: boolean }[];
};

export function listSelectedTestsByCategory(
  categories: string[],
  disabledTests: string[],
): SelectedTestGroup[] {
  return listPlanCategoryGroups(categories, disabledTests).map((group) => ({
    categoryId: group.categoryId,
    label: group.label,
    tests: group.tests
      .filter((test) => test.enabled)
      .map((test) => ({ id: test.id, name: test.name })),
  }));
}

export function listPlanCategoryGroups(
  categories: string[],
  disabledTests: string[],
): PlanCategoryGroup[] {
  const disabled = new Set(disabledTests);
  const groups: PlanCategoryGroup[] = [];
  for (const categoryId of categories) {
    const category = ATTACK_CATALOG.find((item) => item.id === categoryId);
    if (!category) continue;
    const tests = category.tests.map((test) => ({
      id: test.id,
      name: test.name,
      enabled: !disabled.has(test.id),
    }));
    const enabledCount = tests.filter((test) => test.enabled).length;
    if (enabledCount === 0) continue;
    groups.push({
      categoryId,
      label: category.label,
      enabledCount,
      totalCount: tests.length,
      tests,
    });
  }
  return groups;
}

export function estimateAttackPlan(
  profileId: string,
  categories: string[],
  disabledTests: string[],
): { requests: number; runtime: string } {
  const input = {
    selectedEndpointCount: 1,
    profileId: profileId as AttackProfileId,
    customCategories: categories as AttackCategoryId[],
    disabledTestIds: new Set(disabledTests),
  };
  return {
    requests: estimateRequests(input),
    runtime: formatEstimatedRuntime(estimateRuntimeSeconds(input)),
  };
}
