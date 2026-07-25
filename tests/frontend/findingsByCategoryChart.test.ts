import { describe, expect, it } from "vitest";

import { buildFindingsByCategory } from "@/features/scans/FindingsByCategoryChart";
import type { Finding } from "@/shared/types";

function finding(partial: Partial<Finding> & Pick<Finding, "id" | "category">): Finding {
  return {
    scanId: "scan-1",
    projectId: "proj-1",
    targetId: "tgt-1",
    targetName: "Target",
    targetUrl: "https://example.com",
    title: "Finding",
    description: "",
    severity: "high",
    status: "open",
    confidence: 0.9,
    verdict: "vulnerable",
    discoveredAt: "2026-01-01T00:00:00Z",
    evidence: null,
    ...partial,
  };
}

describe("buildFindingsByCategory", () => {
  it("aggregates counts and sorts by count desc", () => {
    const bars = buildFindingsByCategory([
      finding({ id: "1", category: "jailbreak" }),
      finding({ id: "2", category: "prompt_injection" }),
      finding({ id: "3", category: "prompt_injection" }),
      finding({ id: "4", category: "jailbreak" }),
      finding({ id: "5", category: "prompt_injection" }),
    ]);

    expect(bars.map((bar) => ({ id: bar.id, count: bar.count }))).toEqual([
      { id: "prompt_injection", count: 3 },
      { id: "jailbreak", count: 2 },
    ]);
    expect(bars[0]?.label).toBe("Prompt Injection");
  });
});
