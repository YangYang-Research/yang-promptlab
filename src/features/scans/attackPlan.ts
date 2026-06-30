import type {
  AttackCategoryId,
  AttackProfileId,
  ExecutionStrategy,
} from "./attackProfiles";
import {
  payloadStrategyFromDto,
  payloadStrategyToDto,
  type PayloadStrategyConfig,
  type PayloadStrategyDto,
} from "./payloadStrategy";
import type { AttackPlanUiState, PlannerSource } from "./wizardState";
import { attackPlanUiFromPlan } from "./wizardState";

export type AttackGraphNode = {
  category: AttackCategoryId;
  priority: number;
  risk: number;
  confidence: number;
  dependencies: AttackCategoryId[];
  enabled: boolean;
};

export type CategoryRationale = {
  category: AttackCategoryId;
  reason: string;
  priority: number;
  source: string;
};

export type AttackProfileMode = {
  profileId: AttackProfileId;
  categories: AttackCategoryId[];
  executionStrategy: ExecutionStrategy;
  maxAttempts: number;
  reflectionEnabled: boolean;
  adaptivePlanning: boolean;
  payloadStrategy: PayloadStrategyConfig;
};

/** Planner output + user review overrides — persisted in wizard session. */
export type AttackPlanConfig = {
  profileId: AttackProfileId;
  recommendedProfileId: AttackProfileId;
  suggestedCategories: AttackCategoryId[];
  profileModes: AttackProfileMode[];
  customCategories: AttackCategoryId[];
  categories: AttackCategoryId[];
  disabledTests: string[];
  disabledGraphNodes: AttackCategoryId[];
  capabilityGraph: string[];
  attackGraph: AttackGraphNode[];
  executionStrategy: ExecutionStrategy;
  maxAttempts: number;
  reflectionEnabled: boolean;
  adaptivePlanning: boolean;
  rationales: CategoryRationale[];
  confidence: number;
  summary: string;
  riskScore: number;
  riskLevel: string;
  estimatedRequests: number;
  estimatedRuntimeSeconds: number;
  estimatedTokens: number;
  coverageScore: number;
  riskCoverage: number;
  totalTestcases: number;
  payloadStrategy: PayloadStrategyConfig;
  recommendedPayloadStrategy: PayloadStrategyConfig;
  plannerSource: PlannerSource;
};

export type AttackProfileModeDto = {
  profileId: string;
  categories: string[];
  executionStrategy: string;
  maxAttempts: number;
  reflectionEnabled: boolean;
  adaptivePlanning: boolean;
  payloadStrategy: PayloadStrategyDto;
};

export type WizardAttackPlanDto = {
  profileId: string;
  recommendedProfileId: string;
  suggestedCategories: string[];
  profileModes: AttackProfileModeDto[];
  categories: string[];
  disabledTests: string[];
  capabilityGraph: string[];
  attackGraph: Array<{
    category: string;
    priority: number;
    risk: number;
    confidence: number;
    dependencies: string[];
    enabled: boolean;
  }>;
  executionStrategy: string;
  maxAttempts: number;
  reflectionEnabled: boolean;
  adaptivePlanning: boolean;
  rationales: Array<{
    category: string;
    reason: string;
    priority: number;
    source: string;
  }>;
  confidence: number;
  summary: string;
  riskScore: number;
  riskLevel: string;
  estimatedRequests: number;
  estimatedRuntimeSeconds: number;
  estimatedTokens: number;
  coverageScore: number;
  riskCoverage: number;
  totalTestcases: number;
  payloadStrategy: PayloadStrategyDto;
  recommendedPayloadStrategy: PayloadStrategyDto;
  plannerSource: string;
};

export type PlannerAdjustRequest = {
  targetId: string;
  profileId: AttackProfileId;
  categories: AttackCategoryId[];
  disabledTests: string[];
  disabledGraphNodes: AttackCategoryId[];
  executionStrategy?: ExecutionStrategy;
  maxAttempts?: number;
  reflectionEnabled?: boolean;
  adaptivePlanning?: boolean;
  payloadStrategy?: PayloadStrategyDto;
  suggestedCategories?: AttackCategoryId[];
  profileModes?: AttackProfileModeDto[];
  rationales?: Array<{
    category: string;
    reason: string;
    priority: number;
    source: string;
  }>;
  capabilityGraph?: string[];
};

export function normalizeAttackPlan(plan: AttackPlanConfig): AttackPlanConfig {
  const recommendedProfileId = asProfileId(plan.recommendedProfileId ?? plan.profileId);
  const plannerSource =
    plan.plannerSource ??
    (plan.rationales?.some((item) => item.source === "ai_runtime")
      ? "ai_runtime"
      : "target_profile");

  return {
    ...plan,
    recommendedProfileId,
    plannerSource,
    profileModes: plan.profileModes ?? [],
    suggestedCategories: plan.suggestedCategories ?? [],
    rationales: plan.rationales ?? [],
  };
}

const ALL_CATEGORY_IDS = new Set<string>([
  "prompt_injection",
  "system_prompt_extraction",
  "jailbreak",
  "rag_leakage",
  "memory_poisoning",
  "cross_user_leakage",
  "agent_goal_hijacking",
  "tool_abuse",
  "mcp_abuse",
]);

function asCategoryId(value: string): AttackCategoryId | null {
  return ALL_CATEGORY_IDS.has(value) ? (value as AttackCategoryId) : null;
}

function asProfileId(value: string): AttackProfileId {
  if (value === "quick" || value === "standard" || value === "deep" || value === "custom") {
    return value;
  }
  return "standard";
}

function rationalesForActiveCategories(
  rationales: CategoryRationale[],
  categories: AttackCategoryId[],
): CategoryRationale[] {
  const active = new Set(categories);
  return rationales
    .filter((item) => active.has(item.category))
    .sort((a, b) => a.priority - b.priority);
}

/** Rationales for the active profile categories without mutating the stored plan catalog. */
export function resolveActivePlannerRationales(
  plan: Pick<AttackPlanConfig, "rationales">,
  categories: AttackCategoryId[],
): CategoryRationale[] {
  return rationalesForActiveCategories(plan.rationales, categories);
}

function mapCategoryRationales(
  values: WizardAttackPlanDto["rationales"],
): CategoryRationale[] {
  return values
    .map((item) => {
      const category = asCategoryId(item.category);
      if (!category) return null;
      return {
        category,
        reason: item.reason,
        priority: item.priority,
        source: item.source,
      };
    })
    .filter((item): item is CategoryRationale => item !== null);
}

function profileModeFromDto(dto: AttackProfileModeDto): AttackProfileMode | null {
  const profileId = asProfileId(dto.profileId);
  if (profileId === "custom") return null;
  const categories = dto.categories
    .map(asCategoryId)
    .filter((id): id is AttackCategoryId => id !== null);
  if (categories.length === 0) return null;
  return {
    profileId,
    categories,
    executionStrategy: dto.executionStrategy === "agentic" ? "agentic" : "sequential",
    maxAttempts: dto.maxAttempts,
    reflectionEnabled: dto.reflectionEnabled,
    adaptivePlanning: dto.adaptivePlanning,
    payloadStrategy: payloadStrategyFromDto(dto.payloadStrategy),
  };
}

export function profileModeToDto(mode: AttackProfileMode): AttackProfileModeDto {
  return {
    profileId: mode.profileId,
    categories: mode.categories,
    executionStrategy: mode.executionStrategy,
    maxAttempts: mode.maxAttempts,
    reflectionEnabled: mode.reflectionEnabled,
    adaptivePlanning: mode.adaptivePlanning,
    payloadStrategy: payloadStrategyToDto(mode.payloadStrategy),
  };
}

export function getProfileMode(
  plan: Pick<AttackPlanConfig, "profileModes">,
  profileId: AttackProfileId,
): AttackProfileMode | null {
  return (plan.profileModes ?? []).find((mode) => mode.profileId === profileId) ?? null;
}

/** Categories payload for `attack_planner_adjust` — avoids empty custom selection. */
export function resolveCategoriesForAdjust(
  profileId: AttackProfileId,
  planUi: Pick<AttackPlanUiState, "customCategories" | "disabledGraphNodes">,
  attackPlan: Pick<
    AttackPlanConfig,
    "suggestedCategories" | "categories" | "profileModes"
  >,
): AttackCategoryId[] {
  if (profileId !== "custom") {
    const mode = getProfileMode(attackPlan, profileId);
    return mode?.categories ?? attackPlan.categories;
  }
  if (planUi.customCategories.length > 0) {
    return planUi.customCategories;
  }
  if (attackPlan.categories.length > 0) {
    return attackPlan.categories;
  }
  return attackPlan.suggestedCategories.filter((id) => !planUi.disabledGraphNodes.includes(id));
}

export function attackPlanFromDto(dto: WizardAttackPlanDto): AttackPlanConfig {
  const mapCategories = (values: string[]) =>
    values.map(asCategoryId).filter((id): id is AttackCategoryId => id !== null);

  return normalizeAttackPlan({
    profileId: asProfileId(dto.profileId),
    recommendedProfileId: asProfileId(dto.recommendedProfileId ?? dto.profileId),
    suggestedCategories: mapCategories(dto.suggestedCategories ?? []),
    profileModes: (dto.profileModes ?? [])
      .map(profileModeFromDto)
      .filter((mode): mode is AttackProfileMode => mode !== null),
    customCategories: mapCategories(dto.categories),
    categories: mapCategories(dto.categories),
    disabledTests: dto.disabledTests,
    disabledGraphNodes: dto.attackGraph
      .filter((node) => !node.enabled)
      .map((node) => asCategoryId(node.category))
      .filter((id): id is AttackCategoryId => id !== null),
    capabilityGraph: dto.capabilityGraph,
    attackGraph: dto.attackGraph
      .map((node) => {
        const category = asCategoryId(node.category);
        if (!category) return null;
        return {
          category,
          priority: node.priority,
          risk: node.risk,
          confidence: node.confidence,
          dependencies: mapCategories(node.dependencies),
          enabled: node.enabled,
        };
      })
      .filter((node): node is AttackGraphNode => node !== null),
    executionStrategy: dto.executionStrategy === "agentic" ? "agentic" : "sequential",
    maxAttempts: dto.maxAttempts,
    reflectionEnabled: dto.reflectionEnabled,
    adaptivePlanning: dto.adaptivePlanning,
    rationales: mapCategoryRationales(dto.rationales),
    confidence: dto.confidence,
    summary: dto.summary,
    riskScore: dto.riskScore,
    riskLevel: dto.riskLevel,
    estimatedRequests: dto.estimatedRequests,
    estimatedRuntimeSeconds: dto.estimatedRuntimeSeconds,
    estimatedTokens: dto.estimatedTokens,
    coverageScore: dto.coverageScore,
    riskCoverage: dto.riskCoverage,
    totalTestcases: dto.totalTestcases,
    payloadStrategy: payloadStrategyFromDto(dto.payloadStrategy),
    recommendedPayloadStrategy: payloadStrategyFromDto(dto.recommendedPayloadStrategy),
    plannerSource:
      dto.plannerSource === "ai_runtime" ? "ai_runtime" : "target_profile",
  });
}

export { payloadStrategyToDto };
export type { PayloadStrategyConfig, PayloadStrategyDto };

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

export function formatCoverageScore(score: number): string {
  return `${Math.round(score * 100)}%`;
}

export function formatRiskLevel(level: string): string {
  if (level === "high") return "High";
  if (level === "medium") return "Medium";
  if (level === "low") return "Low";
  return level;
}

const WIZARD_PAYLOADS_PER_CATEGORY = 3;
const WIZARD_TESTS_PER_CATEGORY = 3;
const WIZARD_SECONDS_PER_REQUEST = 2.5;
const WIZARD_TOKENS_PER_REQUEST = 480;
const WIZARD_CATALOG_SIZE = 9;

function testPrefixForCategory(category: AttackCategoryId): string {
  switch (category) {
    case "prompt_injection":
      return "pi-";
    case "system_prompt_extraction":
      return "spe-";
    case "jailbreak":
      return "jb-";
    case "rag_leakage":
      return "rag-";
    case "memory_poisoning":
      return "mp-";
    case "cross_user_leakage":
      return "cul-";
    case "agent_goal_hijacking":
      return "agh-";
    case "tool_abuse":
      return "ta-";
    case "mcp_abuse":
      return "mcp-";
    default:
      return "";
  }
}

function categoryRisk(category: AttackCategoryId): number {
  switch (category) {
    case "prompt_injection":
    case "tool_abuse":
    case "mcp_abuse":
      return 90;
    case "jailbreak":
    case "agent_goal_hijacking":
      return 80;
    case "system_prompt_extraction":
    case "memory_poisoning":
      return 70;
    default:
      return 60;
  }
}

export function extractPlannerEndpoint(summary: string): string {
  const prefix = "Plan for ";
  if (!summary.startsWith(prefix)) return "target";

  let endpoint = summary.slice(prefix.length).trim();
  const legacyMetrics = endpoint.search(/: \d+ categor(?:ies|y)/);
  if (legacyMetrics > 0) {
    endpoint = endpoint.slice(0, legacyMetrics).trim();
  }
  return endpoint || "target";
}

export function formatExecutionStrategySummary(
  plan: Pick<AttackPlanConfig, "executionStrategy">,
): string {
  return plan.executionStrategy === "agentic" ? "Agentic" : "Sequential";
}

export function buildPlannerSummaryPreview(_plan: AttackPlanConfig, endpoint: string): string {
  return `Plan for ${endpoint}`;
}

export function computeWizardPlanMetrics(
  plan: AttackPlanConfig,
): Pick<
  AttackPlanConfig,
  | "totalTestcases"
  | "estimatedRequests"
  | "estimatedRuntimeSeconds"
  | "estimatedTokens"
  | "coverageScore"
  | "riskCoverage"
> {
  let requests = 0;
  let totalTestcases = 0;

  for (const category of plan.categories) {
    const prefix = testPrefixForCategory(category);
    const disabledInCategory = plan.disabledTests.filter((id) => id.startsWith(prefix)).length;
    const enabledTests = Math.max(0, WIZARD_TESTS_PER_CATEGORY - disabledInCategory);
    if (enabledTests === 0) continue;
    totalTestcases += enabledTests;
    const ratio = enabledTests / WIZARD_TESTS_PER_CATEGORY;
    requests += Math.round(
      WIZARD_PAYLOADS_PER_CATEGORY * plan.payloadStrategy.variantsPerTest * ratio,
    );
  }

  const executionMultiplier = plan.executionStrategy === "agentic" ? Math.max(1, plan.maxAttempts) : 1;
  const estimatedRequests = requests * executionMultiplier;
  const enabledRisk = plan.attackGraph
    .filter((node) => plan.categories.includes(node.category))
    .reduce((sum, node) => sum + categoryRisk(node.category), 0);
  const totalRisk = plan.attackGraph.reduce((sum, node) => sum + categoryRisk(node.category), 0) || 1;

  return {
    totalTestcases,
    estimatedRequests,
    estimatedRuntimeSeconds: Math.ceil(estimatedRequests * WIZARD_SECONDS_PER_REQUEST),
    estimatedTokens: estimatedRequests * WIZARD_TOKENS_PER_REQUEST,
    coverageScore: plan.categories.length / WIZARD_CATALOG_SIZE,
    riskCoverage: enabledRisk / totalRisk,
  };
}

export function recomputePlanPreview(plan: AttackPlanConfig): AttackPlanConfig {
  const endpoint = extractPlannerEndpoint(plan.summary);
  const metrics = computeWizardPlanMetrics(plan);
  const next = { ...plan, ...metrics };
  return { ...next, summary: buildPlannerSummaryPreview(next, endpoint) };
}

export function plannerAdjustContext(
  plan: AttackPlanConfig,
): Pick<
  PlannerAdjustRequest,
  "suggestedCategories" | "profileModes" | "rationales" | "capabilityGraph"
> {
  return {
    suggestedCategories: plan.suggestedCategories ?? [],
    profileModes: (plan.profileModes ?? []).map(profileModeToDto),
    rationales: plan.rationales.map((item) => ({
      category: item.category,
      reason: item.reason,
      priority: item.priority,
      source: item.source,
    })),
    capabilityGraph: plan.capabilityGraph,
  };
}

export function previewPlanForProfile(
  plan: AttackPlanConfig,
  profileId: AttackProfileId,
  disabledTests: string[],
): AttackPlanConfig {
  if (profileId === "custom") {
    return recomputePlanPreview({ ...plan, profileId, disabledTests });
  }

  const mode = getProfileMode(plan, profileId);
  if (!mode) {
    return recomputePlanPreview({ ...plan, profileId, disabledTests });
  }

  const categories = mode.categories;
  const attackGraph = plan.attackGraph.map((node) => ({
    ...node,
    enabled: categories.includes(node.category),
  }));

  return recomputePlanPreview({
    ...plan,
    profileId,
    categories,
    attackGraph,
    disabledTests,
    disabledGraphNodes: [],
    executionStrategy: mode.executionStrategy,
    maxAttempts: mode.maxAttempts,
    reflectionEnabled: mode.reflectionEnabled,
    adaptivePlanning: mode.adaptivePlanning,
    payloadStrategy: mode.payloadStrategy,
  });
}

export function resolveCategoriesForProfile(
  plan: AttackPlanConfig,
  profileId: AttackProfileId,
): AttackCategoryId[] {
  if (profileId === "custom") {
    return plan.categories;
  }
  return getProfileMode(plan, profileId)?.categories ?? plan.categories;
}

export function plannerSourceFromPlan(plan: AttackPlanConfig): PlannerSource {
  if (plan.plannerSource === "ai_runtime") return "ai_runtime";
  return plan.rationales.some((item) => item.source === "ai_runtime")
    ? "ai_runtime"
    : "target_profile";
}

/** Stable fingerprint of user-adjustable plan fields for customization detection. */
export function planCustomizationKey(plan: AttackPlanConfig): string {
  return JSON.stringify({
    profileId: plan.profileId,
    categories: [...plan.categories].sort(),
    disabledTests: [...plan.disabledTests].sort(),
    disabledGraphNodes: [...plan.disabledGraphNodes].sort(),
    executionStrategy: plan.executionStrategy,
    maxAttempts: plan.maxAttempts,
    reflectionEnabled: plan.reflectionEnabled,
    adaptivePlanning: plan.adaptivePlanning,
    payloadStrategy: plan.payloadStrategy,
  });
}

export type PlannerSummaryBadge = {
  label: string;
  variant: "info" | "warning" | "muted";
};

export function resolvePlannerSummaryBadge(
  plan: AttackPlanConfig,
  planUi: Pick<AttackPlanUiState, "plannerSource" | "profileId">,
): PlannerSummaryBadge {
  const source = planUi.plannerSource ?? plannerSourceFromPlan(plan);
  const profileId = planUi.profileId ?? plan.profileId;

  if (profileId === "custom") {
    return { label: "Customized", variant: "warning" };
  }

  if (source === "ai_runtime") {
    return { label: "AI Planned", variant: "info" };
  }

  return { label: "Suggested", variant: "muted" };
}

export type ProfileModeBadge = {
  label: string;
  variant: "info";
  className: string;
};

export function resolveProfileModeBadge(
  plan: AttackPlanConfig,
  profileId: AttackProfileId,
): ProfileModeBadge | null {
  if (profileId === "custom") {
    return null;
  }
  if (plan.recommendedProfileId === profileId) {
    return {
      label: "AI Recommended",
      variant: "info",
      className:
        "wizard-attack-profile__badge wizard-attack-profile__badge--recommended",
    };
  }
  return {
    label: "AI",
    variant: "info",
    className: "wizard-attack-profile__badge",
  };
}

export function attackPlanUiBaselineFromPlan(plan: AttackPlanConfig): AttackPlanUiState {
  return {
    ...attackPlanUiFromPlan(plan),
    plannerSource: plannerSourceFromPlan(plan),
    suggestedPlanKey: planCustomizationKey(plan),
  };
}

export function syncAttackPlanUiAfterAdjust(
  plan: AttackPlanConfig,
  prev: AttackPlanUiState,
): AttackPlanUiState {
  const synced = attackPlanUiFromPlan(plan);
  return {
    ...synced,
    expandedCategory: prev.expandedCategory,
    plannerSource: prev.plannerSource,
    suggestedPlanKey: prev.suggestedPlanKey,
  };
}
