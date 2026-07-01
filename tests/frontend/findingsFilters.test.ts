import { describe, expect, it } from "vitest";

import { filterFindings } from "@/features/findings/findingsFilters";
import type { Finding, Project, ScanRun } from "@/shared/types";

const project: Project = {
  id: "proj-1",
  name: "Acme",
  description: "",
  status: "active",
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
  targetCount: 1,
  findingCount: 1,
  owner: "team",
};

const scan: ScanRun = {
  id: "scan-1",
  projectId: "proj-1",
  targetId: "target-1",
  name: "Quick scan",
  status: "completed",
  startedAt: "2026-01-01T00:00:00Z",
  completedAt: "2026-01-01T01:00:00Z",
  createdAt: "2026-01-01T00:00:00Z",
};

const finding: Finding = {
  id: "finding-1",
  scanId: "scan-1",
  projectId: "proj-1",
  targetId: "target-1",
  targetName: "Chat API",
  title: "Prompt injection detected",
  description: "System prompt leak via delimiter attack",
  severity: "high",
  category: "prompt_injection",
  status: "open",
  confidence: 0.82,
  verdict: "vulnerable",
  discoveredAt: "2026-01-01T00:30:00Z",
  evidence: {
    payload: "ignore previous instructions",
    explanation: "Model followed injected instruction",
  },
};

describe("filterFindings", () => {
  it("filters by severity and search query", () => {
    const results = filterFindings(
      [finding],
      {
        searchQuery: "delimiter",
        projectId: null,
        scanId: null,
        severity: "high",
        status: null,
      },
      [project],
      [scan],
    );
    expect(results).toHaveLength(1);
  });

  it("excludes findings that do not match project filter", () => {
    const results = filterFindings(
      [finding],
      {
        searchQuery: "",
        projectId: "other-project",
        scanId: null,
        severity: null,
        status: null,
      },
      [project],
      [scan],
    );
    expect(results).toHaveLength(0);
  });

  it("matches project name in search", () => {
    const results = filterFindings(
      [finding],
      {
        searchQuery: "acme",
        projectId: null,
        scanId: null,
        severity: null,
        status: null,
      },
      [project],
      [scan],
    );
    expect(results).toHaveLength(1);
  });
});
