import { createAppError } from "@/shared/errors";

export const MODEL_OPERATION_TIMEOUT_MS = 45_000;

export function withModelOperationTimeout<T>(
  promise: Promise<T>,
  operation: string,
): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = window.setTimeout(() => {
      reject(
        createAppError(
          "IPC",
          `${operation} timed out after ${MODEL_OPERATION_TIMEOUT_MS / 1000} seconds`,
        ),
      );
    }, MODEL_OPERATION_TIMEOUT_MS);

    promise.then(
      (value) => {
        window.clearTimeout(timer);
        resolve(value);
      },
      (error) => {
        window.clearTimeout(timer);
        reject(error);
      },
    );
  });
}
