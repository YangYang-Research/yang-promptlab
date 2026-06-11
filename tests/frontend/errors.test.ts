import { describe, expect, it } from "vitest";

import { createAppError, isAppError, toAppError } from "@/shared/errors";

describe("AppError", () => {
  it("creates typed errors", () => {
    const error = createAppError("IPC", "invoke failed");
    expect(error.code).toBe("IPC");
    expect(error.message).toBe("invoke failed");
  });

  it("normalizes unknown errors", () => {
    const error = toAppError(new Error("boom"));
    expect(error.message).toBe("boom");
    expect(isAppError(error)).toBe(true);
  });
});
