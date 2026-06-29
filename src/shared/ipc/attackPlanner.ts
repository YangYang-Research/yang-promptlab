import { invokeCommand } from "./invoke";
import type { PlannerAdjustRequest, WizardAttackPlanDto } from "@/features/scans/attackPlan";

export const generateAttackPlanForTarget = (targetId: string) =>
  invokeCommand<WizardAttackPlanDto>("planner_generate_from_profile", { targetId });

export const adjustAttackPlan = (request: PlannerAdjustRequest) =>
  invokeCommand<WizardAttackPlanDto>("attack_planner_adjust", { request });
