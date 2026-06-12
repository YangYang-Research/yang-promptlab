import { invoke } from "@tauri-apps/api/core";

import { createAppError, type AppError, type ErrorCode } from "@/shared/errors";

type CommandErrorPayload = {
  code: string;
  message: string;
};

export type HealthResponse = {
  status: string;
  version: string;
};

export type AppInfoResponse = {
  name: string;
  version: string;
  identifier: string;
};

function mapCommandError(payload: CommandErrorPayload): AppError {
  const code = payload.code as ErrorCode;
  return createAppError(code, payload.message);
}

async function invokeCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    if (
      typeof error === "object" &&
      error !== null &&
      "code" in error &&
      "message" in error
    ) {
      throw mapCommandError(error as CommandErrorPayload);
    }

    throw createAppError("IPC", "IPC invocation failed", error);
  }
}

export function healthCheck(): Promise<HealthResponse> {
  return invokeCommand<HealthResponse>("health");
}

export function getAppInfo(): Promise<AppInfoResponse> {
  return invokeCommand<AppInfoResponse>("app_info");
}

// ---------------------------------------------------------------------------
// Domain DTOs (mirror the Rust command responses; timestamps are RFC 3339)
// ---------------------------------------------------------------------------

export type ProjectDto = {
  id: string;
  name: string;
  description: string | null;
  created_at: string;
  updated_at: string;
};

export type TargetDto = {
  id: string;
  project_id: string;
  name: string;
  target_type: string;
  descriptor: unknown;
  created_at: string;
  updated_at: string;
};

export type ScanDto = {
  id: string;
  project_id: string;
  target_id: string | null;
  name: string;
  status: string;
  started_at: string | null;
  completed_at: string | null;
  created_at: string;
  updated_at: string;
};

export type FindingDto = {
  id: string;
  scan_id: string;
  project_id: string;
  target_id: string | null;
  title: string;
  severity: string;
  category: string | null;
  description: string | null;
  evidence: unknown;
  status: string;
  created_at: string;
  updated_at: string;
};

export type ReportDto = {
  id: string;
  project_id: string;
  scan_id: string | null;
  name: string;
  format: string;
  status: string;
  file_path: string | null;
  created_at: string;
  updated_at: string;
};

// ---------------------------------------------------------------------------
// Domain command wrappers (Tauri auto-maps snake_case Rust args -> camelCase JS)
// ---------------------------------------------------------------------------

export const listProjects = () => invokeCommand<ProjectDto[]>("project_list");

export const createProject = (name: string, description?: string | null) =>
  invokeCommand<ProjectDto>("project_create", { name, description: description ?? null });

export const getProject = (id: string) =>
  invokeCommand<ProjectDto>("project_get", { id });

export const deleteProject = (id: string) =>
  invokeCommand<null>("project_delete", { id });

export const listTargets = (projectId: string) =>
  invokeCommand<TargetDto[]>("target_list", { projectId });

export const createTarget = (
  projectId: string,
  name: string,
  targetType: string,
  descriptor?: unknown,
) => invokeCommand<TargetDto>("target_create", { projectId, name, targetType, descriptor });

export const listScans = (projectId: string) =>
  invokeCommand<ScanDto[]>("scan_list", { projectId });

export const createScan = (
  projectId: string,
  name: string,
  targetId?: string | null,
  status?: string | null,
) =>
  invokeCommand<ScanDto>("scan_create", {
    projectId,
    targetId: targetId ?? null,
    name,
    status: status ?? null,
  });

export const listFindings = (scanId: string) =>
  invokeCommand<FindingDto[]>("finding_list", { scanId });

export const generateReport = (
  projectId: string,
  scanId: string,
  format?: string,
  kind?: string,
) =>
  invokeCommand<ReportDto>("report_generate", { projectId, scanId, format, kind });

export const listReports = (projectId: string) =>
  invokeCommand<ReportDto[]>("report_list", { projectId });
