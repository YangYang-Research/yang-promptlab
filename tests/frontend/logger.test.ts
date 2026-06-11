import { describe, expect, it, vi } from "vitest";

import { createLogger } from "@/shared/logging";

describe("logger", () => {
  it("writes info messages when level allows", () => {
    const info = vi.spyOn(console, "info").mockImplementation(() => undefined);
    const log = createLogger("test");

    log.info("hello", { ok: true });

    expect(info).toHaveBeenCalled();
    info.mockRestore();
  });
});
