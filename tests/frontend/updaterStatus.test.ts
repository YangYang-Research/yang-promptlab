import { describe, expect, it } from "vitest";

import { updateDownloadPercent, type UpdateProgressDto } from "@/shared/ipc/updater";

function progress(partial: Partial<UpdateProgressDto>): UpdateProgressDto {
  return {
    phase: "downloading",
    message: "Downloading",
    currentVersion: "0.1.0",
    downloadedBytes: 0,
    totalBytes: 0,
    ...partial,
  };
}

describe("updateDownloadPercent", () => {
  it("returns null when not downloading or total is unknown", () => {
    expect(updateDownloadPercent(progress({ phase: "checking" }))).toBeNull();
    expect(updateDownloadPercent(progress({ totalBytes: 0 }))).toBeNull();
  });

  it("clamps the downloaded ratio to 0-100", () => {
    expect(
      updateDownloadPercent(progress({ downloadedBytes: 25, totalBytes: 100 })),
    ).toBe(25);
    expect(
      updateDownloadPercent(progress({ downloadedBytes: 200, totalBytes: 100 })),
    ).toBe(100);
  });
});
