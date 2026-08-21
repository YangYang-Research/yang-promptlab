import { describe, expect, it } from "vitest";

import { deriveActivity, deriveAttackRuns } from "@/shared/dashboardDerived";
import type { Finding, Project, ScanRun, Target } from "@/shared/types";
import { severityCountSeries, computeProjectSecurityScore } from "@/shared/stats";

function finding(id: string, discoveredAt: string): Finding {
  return {
    id,
    scanId: "scan-1",
    projectId: "proj-1",
    targetId: "target-1",
    targetName: "API",
    targetUrl: "https://example.com",
    title: `Finding ${id}`,
    description: "",
    severity: "high",
    category: "prompt_injection",
    status: "open",
    confidence: 0.9,
    verdict: "vulnerable",
    discoveredAt,
    evidence: null,
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
    retries: partial.retries ?? [],
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
    authKind: "none",
  };
}

describe("deriveAttackRuns", () => {
  it("includes Agent Scan names that are running", () => {
    const runs = deriveAttackRuns(
      [
        scan({ id: "agent-1", status: "running", name: "Agent Scan (deep)" }),
        scan({ id: "done-1", status: "completed", name: "Agent Scan (quick)" }),
      ],
      [target({ id: "target-1", name: "Chat API" })],
      new Map(),
    );

    expect(runs).toHaveLength(1);
    expect(runs[0]?.id).toBe("agent-1");
    expect(runs[0]?.targetName).toBe("Chat API");
    expect(runs[0]?.status).toBe("running");
  });

  it("prefers live status when store status is stale", () => {
    const runs = deriveAttackRuns(
      [scan({ id: "scan-1", status: "pending", name: "Scan (standard)" })],
      [target({ id: "target-1" })],
      new Map([
        [
          "scan-1",
          {
            scan_id: "scan-1",
            status: "running",
            progress_percent: 40,
            completed: 4,
            total: 10,
            attacks_completed: 1,
            attacks_total: 3,
            testcases_completed: 4,
            testcases_total: 10,
            findings_count: 2,
            current_endpoint: null,
            current_test: null,
            started_at: null,
            agent_mode: false,
            current_phase: null,
            current_attempt: null,
            current_retry: null,
            phase_trail: [],
          },
        ],
      ]),
    );

    expect(runs).toHaveLength(1);
    expect(runs[0]?.status).toBe("running");
    expect(runs[0]?.payloadsRun).toBe(4);
    expect(runs[0]?.payloadsTotal).toBe(10);
  });
});

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
        healthScore: null,
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
      [],
      [],
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
      [],
      [],
    );

    expect(activity).toHaveLength(1);
    expect(activity[0]?.timestamp).toBe("2026-07-06T09:30:00.000Z");
    expect(activity[0]?.message).toContain("running");
  });

  it("includes Agent Scan activity", () => {
    const activity = deriveActivity(
      [],
      [scan({ id: "agent-run", status: "running", name: "Agent Scan (deep)" })],
      [],
      [],
      [],
      [],
    );

    expect(activity).toHaveLength(1);
    expect(activity[0]?.message).toContain("Agent Scan (deep)");
  });

  it("includes Retry Scan events from persisted retries", () => {
    const activity = deriveActivity(
      [],
      [
        scan({
          id: "scan-retry",
          status: "running",
          name: "Scan (standard)",
          startedAt: "2026-07-06T09:00:00.000Z",
          retries: [{ at: "2026-07-06T11:00:00.000Z", mode: "continue" }],
        }),
      ],
      [],
      [],
      [],
      [],
    );

    expect(activity.map((item) => item.message)).toEqual([
      "Retry Scan: Scan (standard)",
      "Scan running: Scan (standard)",
    ]);
    expect(activity[0]?.id).toBe("scan-retry-scan-retry-2026-07-06T11:00:00.000Z");
    expect(activity[0]?.timestamp).toBe("2026-07-06T11:00:00.000Z");
  });

  it("includes exported report events", () => {
    const activity = deriveActivity(
      [],
      [],
      [],
      [
        {
          id: "proj-1",
          name: "Demo",
          description: "",
          status: "active",
          createdAt: "2026-06-01T00:00:00.000Z",
          updatedAt: "2026-06-01T00:00:00.000Z",
          targetCount: 0,
          findingCount: 0,
          healthScore: null,
          owner: "",
        },
      ],
      [
        {
          id: "rep-1",
          projectId: "proj-1",
          projectName: "Demo",
          scanId: "scan-1",
          scanName: "Scan (standard)",
          title: "PromptLab - Security Scan Report",
          format: "pdf",
          status: "completed",
          findingCount: 3,
          createdAt: "2026-07-06T13:00:00.000Z",
          sizeBytes: 0,
        },
      ],
      [],
    );

    expect(activity).toHaveLength(1);
    expect(activity[0]?.type).toBe("report");
    expect(activity[0]?.id).toBe("report-rep-1");
    expect(activity[0]?.message).toBe("Exported PDF report: Scan (standard) (Demo)");
    expect(activity[0]?.timestamp).toBe("2026-07-06T13:00:00.000Z");
  });

  it("includes local runtime and model activity", () => {
    const activity = deriveActivity(
      [],
      [],
      [],
      [],
      [],
      [
        {
          id: "local-runtime-1",
          type: "runtime",
          message: "Selected AI Runtime mode: Local",
          timestamp: "2026-07-06T14:00:00.000Z",
        },
        {
          id: "local-model-1",
          type: "model",
          message: "Added model: llama-3",
          timestamp: "2026-07-06T13:30:00.000Z",
        },
      ],
    );

    expect(activity.map((item) => item.message)).toEqual([
      "Selected AI Runtime mode: Local",
      "Added model: llama-3",
    ]);
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

describe("computeProjectSecurityScore", () => {
  it("returns null when there are no targets", () => {
    expect(
      computeProjectSecurityScore({
        findings: [],
        targetCount: 0,
        hasScanned: false,
      }),
    ).toBeNull();
  });

  it("returns null when targets exist but nothing has been scanned", () => {
    expect(
      computeProjectSecurityScore({
        findings: [],
        targetCount: 2,
        hasScanned: false,
      }),
    ).toBeNull();
  });

  it("returns 100 when scanned with no findings", () => {
    expect(
      computeProjectSecurityScore({
        findings: [],
        targetCount: 1,
        hasScanned: true,
      }),
    ).toBe(100);
  });

  it("returns 0 when every finding is critical", () => {
    expect(
      computeProjectSecurityScore({
        findings: [
          { ...finding("a", "2026-07-01T08:00:00.000Z"), severity: "critical" },
          { ...finding("b", "2026-07-01T09:00:00.000Z"), severity: "critical" },
        ],
        targetCount: 1,
        hasScanned: true,
      }),
    ).toBe(0);
  });

  it("scores a mixed severity set between 0 and 100", () => {
    // raw = 16+8+1 = 25, max = 3*16 = 48 → risk ≈ 52% → score ≈ 48
    const score = computeProjectSecurityScore({
      findings: [
        { ...finding("a", "2026-07-01T08:00:00.000Z"), severity: "critical" },
        { ...finding("b", "2026-07-01T09:00:00.000Z"), severity: "high" },
        { ...finding("c", "2026-07-01T10:00:00.000Z"), severity: "info" },
      ],
      targetCount: 1,
      hasScanned: true,
    });
    expect(score).toBe(48);
  });
});
