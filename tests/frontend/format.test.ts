import { describe, expect, it } from "vitest";

import { formatBytes, formatEta, formatSpeed, shortenPromptLabPath } from "@/shared/utils/format";

describe("formatBytes", () => {
  it("formats small and large values", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(5 * 1024 * 1024 * 1024)).toBe("5.0 GB");
  });
});

describe("formatSpeed", () => {
  it("formats bytes per second", () => {
    expect(formatSpeed(2 * 1024 * 1024)).toBe("2.0 MB/s");
    expect(formatSpeed(0)).toBe("—");
  });
});

describe("formatEta", () => {
  it("formats remaining time", () => {
    expect(formatEta(45)).toBe("45s");
    expect(formatEta(125)).toBe("2m 5s");
    expect(formatEta(0)).toBe("done");
  });
});

describe("shortenPromptLabPath", () => {
  const root = "/Users/lethanhphuc/.promptlab";

  it("shortens root to tilde form", () => {
    expect(shortenPromptLabPath(root, root)).toBe("~/.promptlab");
  });

  it("shortens paths under root", () => {
    expect(shortenPromptLabPath(`${root}/workspaces/promptlab.db`, root)).toBe(
      "~/.promptlab/workspaces/promptlab.db",
    );
  });

  it("leaves unrelated paths unchanged", () => {
    expect(shortenPromptLabPath("/tmp/other", root)).toBe("/tmp/other");
  });
});
