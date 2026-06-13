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
  endpointIds: string[];
};

export type DiscoveryScanPlaybook = {
  seedUrl?: string;
  pagesFetched?: number;
  pagesFailed?: number;
  linksExtracted?: number;
  probesSent?: number;
  durationMs?: number;
  endpointCount?: number;
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
    endpointIds: asStringArray(obj.endpoint_ids),
  };
}

export function parseDiscoveryPlaybook(playbook: unknown): DiscoveryScanPlaybook | null {
  const obj = asRecord(playbook);
  if (!obj) return null;
  if (obj.profile || obj.categories) return null;
  return {
    seedUrl: typeof obj.seed_url === "string" ? obj.seed_url : undefined,
    pagesFetched: typeof obj.pages_fetched === "number" ? obj.pages_fetched : undefined,
    pagesFailed: typeof obj.pages_failed === "number" ? obj.pages_failed : undefined,
    linksExtracted: typeof obj.links_extracted === "number" ? obj.links_extracted : undefined,
    probesSent: typeof obj.probes_sent === "number" ? obj.probes_sent : undefined,
    durationMs: typeof obj.duration_ms === "number" ? obj.duration_ms : undefined,
    endpointCount: typeof obj.endpoint_count === "number" ? obj.endpoint_count : undefined,
  };
}

export function isDiscoveryScanName(name: string): boolean {
  return name.startsWith("Discovery:");
}

export function isAttackScanName(name: string): boolean {
  return name.startsWith("Scan (");
}

export function profileLabel(profileId: string): string {
  return ATTACK_PROFILES.find((profile) => profile.id === profileId)?.label ?? profileId;
}

export function listSelectedTests(categories: string[], disabledTests: string[]): string[] {
  const disabled = new Set(disabledTests);
  const tests: string[] = [];
  for (const categoryId of categories) {
    const category = ATTACK_CATALOG.find((item) => item.id === categoryId);
    if (!category) continue;
    for (const test of category.tests) {
      if (!disabled.has(test.id)) tests.push(`${category.label}: ${test.name}`);
    }
  }
  return tests;
}

export function estimateAttackPlan(
  endpointCount: number,
  profileId: string,
  categories: string[],
  disabledTests: string[],
): { requests: number; runtime: string } {
  const input = {
    selectedEndpointCount: endpointCount,
    profileId: profileId as AttackProfileId,
    customCategories: categories as AttackCategoryId[],
    disabledTestIds: new Set(disabledTests),
  };
  return {
    requests: estimateRequests(input),
    runtime: formatEstimatedRuntime(estimateRuntimeSeconds(input)),
  };
}

export function countDiscoveryStats(endpoints: Array<{ kind: string }>) {
  return {
    total: endpoints.length,
    ai: endpoints.filter((endpoint) => endpoint.kind === "ai_endpoint").length,
    graphql: endpoints.filter((endpoint) => endpoint.kind === "graphql").length,
    openapi: endpoints.filter((endpoint) => endpoint.kind === "openapi").length,
    javascript: endpoints.filter((endpoint) => endpoint.kind === "javascript").length,
    manual: endpoints.filter(
      (endpoint) => endpoint.kind === "manual" || endpoint.kind.includes("manual"),
    ).length,
  };
}
