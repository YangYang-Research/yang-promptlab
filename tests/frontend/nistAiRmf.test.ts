import { describe, expect, it } from "vitest";

import {
  matchesNistBrowse,
  nistFunctionsForTechnique,
  nistRefsForTechnique,
} from "@/features/attack-catalog/nistAiRmf";

describe("nistAiRmf", () => {
  it("maps prompt injection to MAP + MEASURE", () => {
    const refs = nistRefsForTechnique({
      categoryId: "prompt_injection",
      owasp: "LLM01",
    });
    expect(refs.map((ref) => ref.label)).toEqual(
      expect.arrayContaining(["MAP 1.5", "MEASURE 2.3"]),
    );
    expect(nistFunctionsForTechnique({
      categoryId: "prompt_injection",
      owasp: "LLM01",
    })).toEqual(expect.arrayContaining(["map", "measure"]));
  });

  it("maps MCP techniques to GOVERN + MANAGE", () => {
    const refs = nistRefsForTechnique({
      categoryId: "mcp_abuse",
      owasp: "MCP07",
    });
    expect(refs.some((ref) => ref.functionId === "govern")).toBe(true);
    expect(refs.some((ref) => ref.functionId === "manage")).toBe(true);
  });

  it("filters by NIST function family", () => {
    const row = { categoryId: "tool_abuse", owasp: "LLM08" };
    expect(matchesNistBrowse(row, "all")).toBe(true);
    expect(matchesNistBrowse(row, "govern")).toBe(true);
    expect(matchesNistBrowse(row, "map")).toBe(false);
  });
});
