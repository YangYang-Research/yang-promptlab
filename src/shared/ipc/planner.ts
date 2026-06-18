import { invokeCommand } from "./invoke";

export type PlannerMode = "deterministic" | "local_llm";

export type PlannerGenerateRequest = {
  endpointIds: string[];
  mode: PlannerMode;
};

export type CategoryRationaleDto = {
  category: string;
  reason: string;
  priority: number;
  source: string;
};

export type AttackPlanDto = {
  mode: PlannerMode;
  profileId: string;
  categories: string[];
  disabledTests: string[];
  rationales: CategoryRationaleDto[];
  confidence: number;
  summary: string;
  llmRationale: string | null;
};

export function generateAttackPlan(request: PlannerGenerateRequest): Promise<AttackPlanDto> {
  return invokeCommand<AttackPlanDto>("planner_generate", { request });
}
