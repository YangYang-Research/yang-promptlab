import { describe, expect, it } from "vitest";

import { deriveActivity } from "@/shared/dashboardDerived";
import type { Finding, Project, ScanRun, Target } from "@/shared/types";
import { severityCountSeries } from "@/shared/stats";

function finding(id: string, discoveredAt: string): Finding {
  return {
    id,
    scanId: "scan-1",
    projectId: "proj-1",
    targetId: "target-1",
    targetName: "API",
    title: `Finding ${id}`,
    description: "",
    severity: "high",
    category: "prompt_injection",
    status: "open",
    confidence: 0.9,
    verdict: "vulnerable",
    discoveredAt,
  };
}

function scan(partial: Partial<ScanRun> & Pick<ScanRun, "id" | "status">): ScanRun {
  return {
    id: partial.id,
    projectId: "proj-1",
    targetId: "target-1",
    name: partial.name ?? "Scan (wizard)",
    status: partial.status,
    startedAt: partial.startedAt ?? "2026-07-01T10:00:00.000Z",
    completedAt: partial.completedAt ?? null,
    createdAt: partial.createdAt ?? "2026-07-01T09:00:00.000Z",
  };
}

function target(partial: Partial<Target> & Pick<Target, "id">): Target {
  return {
    id: partial.id,
    projectId: "proj-1",
    name: partial.name ?? "Target",
    url: "https://example.com",
    type: "api",
    providerLabel: null,
    status: partial.status ?? "pending",
    createdAt: partial.createdAt ?? "2026-06-30T08:00:00.000Z",
    lastScanAt: null,
    fingerprint: null,
    tags: [],
    authType: "none",
  };
}

describe("deriveActivity", () => {
  it("sorts activity newest first using real timestamps", () => {
    const projects: Project[] = [
      {
        id: "proj-1",
        name: "Demo",
        description: "",
        status: "active",
        createdAt: "2026-06-01T00:00:00.000Z",
        updatedAt: "2026-06-01T00:00:00.000Z",
        targetCount: 1,
        findingCount: 2,
        owner: "",
      },
    ];

    const activity = deriveActivity(
      [
        finding("f-old", "2026-07-01T08:00:00.000Z"),
        finding("f-new", "2026-07-06T12:00:00.000Z"),
      ],
      [scan({ id: "scan-1", status: "completed", completedAt: "2026-07-05T10:00:00.000Z" })],
      [target({ id: "target-1", createdAt: "2026-06-30T08:00:00.000Z" })],
      projects,
    );

    expect(activity[0]?.id).toBe("finding-f-new");
    expect(activity[1]?.id).toBe("scan-scan-1");
    expect(activity[2]?.id).toBe("finding-f-old");
  });

  it("includes running scans with startedAt timestamp", () => {
    const activity = deriveActivity(
      [],
      [scan({ id: "scan-run", status: "running", startedAt: "2026-07-06T09:30:00.000Z" })],
      [],
      [],
    );

    expect(activity).toHaveLength(1);
    expect(activity[0]?.timestamp).toBe("2026-07-06T09:30:00.000Z");
    expect(activity[0]?.message).toContain("running");
  });
});

describe("severityCountSeries", () => {
  it("returns all severities in priority order", () => {
    const series = severityCountSeries([
      finding("a", "2026-07-01T08:00:00.000Z"),
      {
        ...finding("b", "2026-07-01T09:00:00.000Z"),
        severity: "critical",
      },
    ]);

    expect(series).toEqual([
      { severity: "critical", label: "Critical", count: 1 },
      { severity: "high", label: "High", count: 1 },
      { severity: "medium", label: "Medium", count: 0 },
      { severity: "low", label: "Low", count: 0 },
      { severity: "info", label: "Info", count: 0 },
    ]);
  });
});
