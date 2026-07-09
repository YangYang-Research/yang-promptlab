import { describe, expect, it } from "vitest";

import { formatScanConsoleLine } from "@/features/scans/scanConsoleFormat";

describe("formatScanConsoleLine", () => {
  it("formats endpoint path and status", () => {
    const line = formatScanConsoleLine({
      scanId: "scan-1",
      timestamp: "2026-07-09T15:04:05Z",
      level: "INFO",
      message: "Probe sent",
      endpoint: "https://api.example.com/v1/chat",
      statusCode: 200,
      latency: 42,
    });

    expect(line).toContain("Probe sent");
    expect(line).toContain("@ /v1/chat");
    expect(line).toContain("→ 200 42ms");
  });
});
