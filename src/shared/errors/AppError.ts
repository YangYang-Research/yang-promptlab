export type ErrorCode =
  | "INTERNAL"
  | "CONFIG"
  | "IO"
  | "NOT_FOUND"
  | "INVALID_INPUT"
  | "UNAUTHORIZED"
  | "PLUGIN"
  | "STORAGE"
  | "IPC"
  | "UNKNOWN";

export type AppError = {
  code: ErrorCode;
  message: string;
  cause?: unknown;
};

export function isAppError(value: unknown): value is AppError {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const candidate = value as Partial<AppError>;
  return typeof candidate.code === "string" && typeof candidate.message === "string";
}

export function createAppError(
  code: ErrorCode,
  message: string,
  cause?: unknown,
): AppError {
  return { code, message, cause };
}

export function toAppError(error: unknown): AppError {
  if (isAppError(error)) {
    return error;
  }

  if (error instanceof Error) {
    return createAppError("UNKNOWN", error.message, error);
  }

  return createAppError("UNKNOWN", "An unexpected error occurred", error);
}
