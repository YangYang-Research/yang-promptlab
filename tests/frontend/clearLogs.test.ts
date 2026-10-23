import { beforeEach, describe, expect, it, vi } from "vitest";

import { clearLogs, listLogFiles } from "@/shared/ipc/environment";
import { invokeCommand } from "@/shared/ipc/invoke";

vi.mock("@/shared/ipc/invoke", () => ({ invokeCommand: vi.fn() }));

describe("clearLogs", () => {
  beforeEach(() => vi.resetAllMocks());

  it("invokes the dedicated log command, not the workspace reset", async () => {
    vi.mocked(invokeCommand).mockResolvedValue(undefined);
    await clearLogs();
    expect(invokeCommand).toHaveBeenCalledExactlyOnceWith("logs_clear");
  });

  it("propagates failures so the card can display an error", async () => {
    const error = new Error("Logs are not writable");
    vi.mocked(invokeCommand).mockRejectedValue(error);
    await expect(clearLogs()).rejects.toBe(error);
  });

  it("returns log names, paths, and byte sizes for the storage card", async () => {
    const files = [
      { name: "app.log", path: "/logs/app.log", sizeBytes: 2048 },
      { name: "scan-console/scan-1.log", path: "/logs/scan-console/scan-1.log", sizeBytes: 0 },
    ];
    vi.mocked(invokeCommand).mockResolvedValue(files);
    await expect(listLogFiles()).resolves.toEqual(files);
    expect(invokeCommand).toHaveBeenCalledExactlyOnceWith("logs_list_files");
  });

  it("propagates listing failures instead of reporting an empty list", async () => {
    const error = new Error("Cannot read log directory");
    vi.mocked(invokeCommand).mockRejectedValue(error);
    await expect(listLogFiles()).rejects.toBe(error);
  });
});