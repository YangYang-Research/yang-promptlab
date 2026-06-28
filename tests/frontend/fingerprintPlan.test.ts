import { describe, expect, it } from "vitest";

import {
  aggregateAttackSuggestions,
  mapFingerprintCategory,
  platformLabel,
} from "@/features/scans/fingerprintPlan";
import type { EndpointDto } from "@/shared/ipc/client";

function endpoint(id: string, category: string): EndpointDto {
  return {
    id,
    scan_id: "scan-1",
    target_id: "target-1",
    url: `https://example.com/${id}`,
    kind: "ai_endpoint",
    method: "POST",
    confidence: 0.9,
    evidence: null,
    source_url: null,
    discovered_at: "2026-01-01T00:00:00Z",
    endpoint_type: "ai_chat",
    ai_framework: "dify",
    risk_score: 70,
    metadata_confidence: 0.9,
    discovery_source: "discovery",
    auth_required: false,
    metadata: null,
    attack_recommendations: [{ category, reason: "test", priority: 1 }],
  };
}

describe("mapFingerprintCategory", () => {
  it("maps system prompt leakage to extraction", () => {
    expect(mapFingerprintCategory("system_prompt_leakage")).toBe("system_prompt_extraction");
  });
});

describe("aggregateAttackSuggestions", () => {
  it("collects mapped categories from selected endpoints", () => {
    const result = aggregateAttackSuggestions(
      [endpoint("a", "mcp_abuse"), endpoint("b", "tool_abuse")],
      ["a", "b"],
    );
    expect(result.categories).toContain("mcp_abuse");
    expect(result.categories).toContain("tool_abuse");
  });
});

describe("platformLabel", () => {
  it("formats known platforms", () => {
    expect(platformLabel("openwebui")).toBe("OpenWebUI");
    expect(platformLabel("langflow")).toBe("Langflow");
  });
});
