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
  targetUrl: "https://api.example.com/v1/chat",
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

const mediumFinding: Finding = {
  ...finding,
  id: "finding-2",
  severity: "medium",
  status: "confirmed",
};

describe("filterFindings", () => {
  it("filters by severity and search query", () => {
    const results = filterFindings(
      [finding],
      {
        searchQuery: "delimiter",
        projectIds: [],
        scanIds: [],
        severities: ["high"],
        statuses: [],
      },
      [project],
      [scan],
    );
    expect(results).toHaveLength(1);
  });

  it("supports multiple severities", () => {
    const results = filterFindings(
      [finding, mediumFinding],
      {
        searchQuery: "",
        projectIds: [],
        scanIds: [],
        severities: ["high", "medium"],
        statuses: [],
      },
      [project],
      [scan],
    );
    expect(results).toHaveLength(2);
  });

  it("excludes findings that do not match project filter", () => {
    const results = filterFindings(
      [finding],
      {
        searchQuery: "",
        projectIds: ["other-project"],
        scanIds: [],
        severities: [],
        statuses: [],
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
        projectIds: [],
        scanIds: [],
        severities: [],
        statuses: [],
      },
      [project],
      [scan],
    );
    expect(results).toHaveLength(1);
  });
});
