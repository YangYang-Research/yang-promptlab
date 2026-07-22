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
  /** Fresh Step 2 capability-probe HTTP console (before Yazg classification). */
  probeConsole?: VerificationConsoleEntryDto | null;
};

export type VerifyHttpSnapshot = {
  console: VerificationConsoleEntryDto;
  responseText: string;
  requestBody: string;
  responseTimeMs: number;
  statusCode: number;
};

export type TargetProfileConnectVerifyResponse = {
  success: boolean;
  console: VerificationConsoleEntryDto;
  message: string;
  connectSnapshot: VerifyHttpSnapshot | null;
};

export type TargetProfileCapabilityVerifyResponse = {
  success: boolean;
  console: VerificationConsoleEntryDto;
  message: string;
  capabilitySnapshot: VerifyHttpSnapshot | null;
  profile: TargetProfileDto;
};

export const listTargetProfileTemplates = () =>
  invokeCommand<TargetProfileDto[]>("target_profile_list_templates");

export const getTargetProfile = (targetId: string) =>
  invokeCommand<TargetProfileDto>("target_profile_get", { targetId });

export const saveTargetProfile = (targetId: string, profile: Record<string, unknown>) =>
  invokeCommand<{ id: string }>("target_profile_save", { targetId, profile });

export const verifyTargetProfileConnect = (
  targetId: string,
  profile: Record<string, unknown>,
  options?: {
    auth?: Record<string, unknown> | null;
    authHeaders?: Record<string, string> | null;
  },
) =>
  invokeCommand<TargetProfileConnectVerifyResponse>("target_profile_verify_connect", {
    targetId,
    profile,
    auth: options?.auth ?? null,
    authHeaders: options?.authHeaders ?? null,
  });

/** Step 2a — capability probe HTTP only (render console before Yazg). */
export const verifyTargetProfileCapability = (
  targetId: string,
  profile: Record<string, unknown>,
  options?: {
    auth?: Record<string, unknown> | null;
    authHeaders?: Record<string, string> | null;
  },
) =>
  invokeCommand<TargetProfileCapabilityVerifyResponse>("target_profile_verify_capability", {
    targetId,
    profile,
    auth: options?.auth ?? null,
    authHeaders: options?.authHeaders ?? null,
  });

/** Step 2b — Yazg classification of an already-captured capability probe. */
export const verifyTargetProfileAiClassify = (
  targetId: string,
  profile: Record<string, unknown>,
  capabilitySnapshot: VerifyHttpSnapshot,
) =>
  invokeCommand<TargetProfileVerifyResponse>("target_profile_verify_ai_classify", {
    targetId,
    profile,
    capabilitySnapshot,
  });

/** Combined Step 2 (probe + classify). Prefer the split APIs for progressive console logs. */
export const verifyTargetProfileAi = (
  targetId: string,
  profile: Record<string, unknown>,
  options?: {
    auth?: Record<string, unknown> | null;
    authHeaders?: Record<string, string> | null;
  },
) =>
  invokeCommand<TargetProfileVerifyResponse>("target_profile_verify_ai", {
    targetId,
    profile,
    auth: options?.auth ?? null,
    authHeaders: options?.authHeaders ?? null,
  });

export const verifyTargetProfile = (
  targetId: string,
  profile: Record<string, unknown>,
  options?: {
    auth?: Record<string, unknown> | null;
    authHeaders?: Record<string, string> | null;
  },
) =>
  invokeCommand<TargetProfileVerifyResponse>("target_profile_verify", {
    targetId,
    profile,
    auth: options?.auth ?? null,
    authHeaders: options?.authHeaders ?? null,
  });
