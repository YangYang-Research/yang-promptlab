import { describe, expect, it } from "vitest";

import {
  buildReportScanRows,
  buildScanExportRows,
  findLatestHtmlReport,
} from "@/features/reports/reportDownloads";

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

describe("findLatestHtmlReport", () => {
  const reports = [
    {
      id: "html-old",
      scanId: "scan-1",
      format: "html",
      status: "completed",
      createdAt: "2026-01-01T00:00:00Z",
    },
    {
      id: "pdf-new",
      scanId: "scan-1",
      format: "pdf",
      status: "completed",
      createdAt: "2026-01-03T00:00:00Z",
    },
    {
      id: "html-new",
      scanId: "scan-1",
      format: "html",
      status: "completed",
      createdAt: "2026-01-02T00:00:00Z",
    },
    {
      id: "html-failed",
      scanId: "scan-1",
      format: "html",
      status: "failed",
      createdAt: "2026-01-04T00:00:00Z",
    },
  ];

  it("returns the newest completed HTML report for the selected scan", () => {
    expect(findLatestHtmlReport(reports, "scan-1")?.id).toBe("html-new");
  });

  it("returns null when the scan has no completed HTML report", () => {
    expect(findLatestHtmlReport(reports, "scan-2")).toBeNull();
  });
});

describe("buildReportScanRows", () => {
  const base = {
    projects: [
      { id: "proj-1", name: "Acme" },
      { id: "proj-2", name: "Globex" },
    ],
    scans: [
      { id: "scan-1", projectId: "proj-1", name: "Quick scan" },
      { id: "scan-2", projectId: "proj-2", name: "Deep scan" },
      { id: "scan-3", projectId: "proj-1", name: "No report scan" },
    ],
    findings: [{ scanId: "scan-1" }, { scanId: "scan-1" }, { scanId: "scan-2" }],
  };

  it("lists only scans that already have reports, newest first", () => {
    const rows = buildReportScanRows({
      ...base,
      reports: [
        {
          id: "report-html-1",
          scanId: "scan-1",
          format: "html",
          status: "completed",
          createdAt: "2026-01-01T00:00:00Z",
        },
        {
          id: "report-pdf-1",
          scanId: "scan-1",
          format: "pdf",
          status: "completed",
          createdAt: "2026-01-02T00:00:00Z",
        },
        {
          id: "report-csv-2",
          scanId: "scan-2",
          format: "csv",
          status: "completed",
          createdAt: "2026-01-05T00:00:00Z",
        },
      ],
    });

    expect(rows.map((row) => row.scanId)).toEqual(["scan-2", "scan-1"]);
    expect(rows[1]).toMatchObject({
      projectName: "Acme",
      scanName: "Quick scan",
      reportId: "report-html-1",
      findingCount: 2,
      reportCount: 2,
      formats: ["html", "pdf"],
      lastGeneratedAt: "2026-01-02T00:00:00Z",
    });
  });

  it("ignores reports without a scan association", () => {
    const rows = buildReportScanRows({
      ...base,
      reports: [
        {
          id: "orphan-report",
          scanId: null,
          format: "html",
          status: "completed",
          createdAt: "2026-01-09T00:00:00Z",
        },
      ],
    });

    expect(rows).toHaveLength(0);
  });
});
