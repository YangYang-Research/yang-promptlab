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
