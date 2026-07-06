import { describe, expect, it } from "vitest";

import {
  buildSeverityBreakdown,
  resolveFindingSubcategory,
} from "@/features/scans/resultsSeverityBreakdown";
import type { Finding } from "@/shared/types";

function sampleFinding(overrides: Partial<Finding> = {}): Finding {
  return {
    id: "f1",
    scanId: "scan-1",
    projectId: "project-1",
    targetId: "target-1",
    targetName: "api.example.com",
    title: "Prompt Injection: Direct instruction override",
    description: "test",
    severity: "high",
    category: "prompt_injection",
    status: "open",
    confidence: 0.9,
    verdict: "vulnerable",
    discoveredAt: "2026-01-01T00:00:00Z",
    evidence: { payload_id: "pi-direct-override" },
    ...overrides,
  };
}

describe("resolveFindingSubcategory", () => {
  it("maps payload_id to attack test name", () => {
    expect(resolveFindingSubcategory(sampleFinding())).toBe("Direct instruction override");
  });

  it("normalizes mutated payload variant ids to the source test name", () => {
    expect(
      resolveFindingSubcategory(
        sampleFinding({
          evidence: {
            payload_id: "pi-direct-override:50cb9696-d868-4e53-9031-9bff32eb8701",
          },
        }),
      ),
    ).toBe("Direct instruction override");
  });

  it("falls back to title suffix when payload_id is missing", () => {
    expect(
      resolveFindingSubcategory(
        sampleFinding({
          evidence: {},
          title: "Jailbreak: DAN roleplay",
        }),
      ),
    ).toBe("DAN roleplay");
  });
});

describe("buildSeverityBreakdown", () => {
  it("groups findings by category and subcategory", () => {
    const breakdown = buildSeverityBreakdown(
      [
        sampleFinding(),
        sampleFinding({
          id: "f2",
          evidence: { payload_id: "pi-indirect-task" },
          title: "Prompt Injection: Indirect task injection",
        }),
        sampleFinding({
          id: "f3",
          category: "jailbreak",
          evidence: { payload_id: "jb-dan" },
          title: "Jailbreak: DAN roleplay",
        }),
      ],
      "high",
      (id) => id.replace(/_/g, " "),
    );

    expect(breakdown).toHaveLength(2);
    expect(breakdown[0]?.categoryId).toBe("jailbreak");
    expect(breakdown[0]?.subcategories).toEqual([{ label: "DAN roleplay", count: 1 }]);
    expect(breakdown[1]?.categoryId).toBe("prompt_injection");
    expect(breakdown[1]?.subcategories).toEqual([
      { label: "Direct instruction override", count: 1 },
      { label: "Indirect task injection", count: 1 },
    ]);
  });
});
