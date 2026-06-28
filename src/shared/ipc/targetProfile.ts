import { invokeCommand } from "./invoke";
import type {
  TargetProfileDto,
  VerificationConsoleEntryDto,
} from "@/features/scans/targetProfile";

export type TargetProfileVerifyResponse = {
  verified: boolean;
  profile: TargetProfileDto;
  console: VerificationConsoleEntryDto;
  message: string;
};

export const listTargetProfileTemplates = () =>
  invokeCommand<TargetProfileDto[]>("target_profile_list_templates");

export const getTargetProfile = (targetId: string) =>
  invokeCommand<TargetProfileDto>("target_profile_get", { targetId });

export const saveTargetProfile = (targetId: string, profile: Record<string, unknown>) =>
  invokeCommand<{ id: string }>("target_profile_save", { targetId, profile });

export const verifyTargetProfile = (targetId: string, profile: Record<string, unknown>) =>
  invokeCommand<TargetProfileVerifyResponse>("target_profile_verify", { targetId, profile });

export const generateAttackPlanFromProfile = (targetId: string, mode = "deterministic") =>
  invokeCommand<import("./planner").AttackPlanDto>("planner_generate_from_profile", {
    targetId,
    mode,
  });
