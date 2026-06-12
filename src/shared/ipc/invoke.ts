import { invoke } from "@tauri-apps/api/core";

import { createAppError, type AppError, type ErrorCode } from "@/shared/errors";

type CommandErrorPayload = {
  code: string;
  message: string;
};

function mapCommandError(payload: CommandErrorPayload): AppError {
  const code = payload.code as ErrorCode;
  return createAppError(code, payload.message);
}

/** Typed Tauri invoke with IPC error envelope mapping. */
export async function invokeCommand<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
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
