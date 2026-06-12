import { describe, expect, it } from "vitest";

import { buildScanExportRows } from "@/features/reports/reportDownloads";

describe("buildScanExportRows", () => {
  it("includes scans that have findings", () => {
    const rows = buildScanExportRows({
      projects: [{ id: "proj-1", name: "Acme" }],
      scans: [{ id: "scan-1", projectId: "proj-1", name: "Quick scan" }],
      findings: [{ scanId: "scan-1" }, { scanId: "scan-1" }],
      reports: [{ scanId: "scan-1", createdAt: "2026-01-02T00:00:00Z" }],
    });

    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({
      scanId: "scan-1",
      projectName: "Acme",
      findingCount: 2,
      lastGeneratedAt: "2026-01-02T00:00:00Z",
    });
  });

  it("excludes scans without findings", () => {
    const rows = buildScanExportRows({
      projects: [{ id: "proj-1", name: "Acme" }],
      scans: [{ id: "scan-1", projectId: "proj-1", name: "Empty scan" }],
      findings: [],
      reports: [],
    });

    expect(rows).toHaveLength(0);
  });
});
