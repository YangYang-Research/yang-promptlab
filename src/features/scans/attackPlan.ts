import type {
  AttackCategoryId,
  AttackProfileId,
  ExecutionStrategy,
} from "./attackProfiles";

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
    rationales: dto.rationales
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
  };
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

export function formatCoverageScore(score: number): string {
  return `${Math.round(score * 100)}%`;
}

export function formatRiskLevel(level: string): string {
  if (level === "high") return "High";
  if (level === "medium") return "Medium";
  if (level === "low") return "Low";
  return level;
}
