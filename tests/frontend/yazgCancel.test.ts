import { describe, expect, it } from "vitest";

import { isYazgCancelledError } from "@/shared/ipc/yazg";
import { createAppError } from "@/shared/errors";

describe("isYazgCancelledError", () => {
  it("matches the IPC cancelled envelope", () => {
    expect(isYazgCancelledError(createAppError("INVALID_INPUT", "cancelled"))).toBe(
      true,
    );
  });

  it("matches wrapped cancel wording", () => {
    expect(isYazgCancelledError(new Error("request cancelled"))).toBe(true);
    expect(isYazgCancelledError(new Error("Canceled by user"))).toBe(true);
  });

  it("ignores ordinary failures", () => {
    expect(isYazgCancelledError(createAppError("INTERNAL", "timeout"))).toBe(false);
    expect(isYazgCancelledError("rate limited")).toBe(false);
  });
});
