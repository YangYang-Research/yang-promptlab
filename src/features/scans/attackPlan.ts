import type {
  AttackCategoryId,
  AttackProfileId,
  ExecutionStrategy,
} from "./attackProfiles";
import { getProfile } from "./attackProfiles";
import {
  payloadStrategyForAttackProfile,
  payloadStrategyFromDto,
  payloadStrategyToDto,
  type PayloadStrategyConfig,
  type PayloadStrategyDto,
} from "./payloadStrategy";
import type { AttackPlanUiState } from "./wizardState";

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

/** Planner output + user review overrides — persisted in wizard session. */
export type AttackPlanConfig = {
  profileId: AttackProfileId;
  suggestedCategories: AttackCategoryId[];
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
};

export type WizardAttackPlanDto = {
  profileId: string;
  suggestedCategories: string[];
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
};

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

/** Categories payload for `attack_planner_adjust` — avoids empty custom selection. */
export function resolveCategoriesForAdjust(
  profileId: AttackProfileId,
  planUi: Pick<AttackPlanUiState, "customCategories" | "disabledGraphNodes">,
  attackPlan: Pick<AttackPlanConfig, "suggestedCategories" | "categories">,
): AttackCategoryId[] {
  if (profileId !== "custom") {
    return attackPlan.suggestedCategories;
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

  return {
    profileId: asProfileId(dto.profileId),
    suggestedCategories: mapCategories(dto.suggestedCategories),
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
    rationales: rationalesForActiveCategories(
      dto.rationales
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
        .filter((item): item is CategoryRationale => item !== null),
      mapCategories(dto.categories),
    ),
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
  };
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
  const rationales = rationalesForActiveCategories(plan.rationales, plan.categories);
  const next = { ...plan, ...metrics, rationales };
  return { ...next, summary: buildPlannerSummaryPreview(next, endpoint) };
}

export function previewPlanForProfile(
  plan: AttackPlanConfig,
  profileId: AttackProfileId,
  disabledTests: string[],
): AttackPlanConfig {
  if (profileId === "custom") {
    return recomputePlanPreview({ ...plan, profileId, disabledTests });
  }

  const preset = getProfile(profileId);
  const categories = plan.suggestedCategories.filter((id) => preset.categories.includes(id));
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
    payloadStrategy: payloadStrategyForAttackProfile(profileId, plan.recommendedPayloadStrategy),
  });
}

export function resolveCategoriesForProfile(
  plan: AttackPlanConfig,
  profileId: AttackProfileId,
): AttackCategoryId[] {
  if (profileId === "custom") {
    return plan.categories;
  }
  const preset = getProfile(profileId);
  return plan.suggestedCategories.filter((id) => preset.categories.includes(id));
}
