import { describe, expect, it } from "vitest";

import { agentLogName, eventBelongsToLogFile } from "@/features/settings/logFileView";

describe("eventBelongsToLogFile", () => {
  it("keeps every category on app.log", () => {
    expect(eventBelongsToLogFile("agent", "app.log")).toBe(true);
    expect(eventBelongsToLogFile("models", "app.log")).toBe(true);
  });

  it("keeps only the matching category on a dedicated file", () => {
    expect(eventBelongsToLogFile("agent", "agents.log")).toBe(true);
    expect(eventBelongsToLogFile("models", "agents.log")).toBe(false);
    expect(eventBelongsToLogFile("application", "agents.log")).toBe(false);
  });

  it("reads the agent display name", () => {
    expect(agentLogName({ agentName: "GeneratePromptAgent", category: "agent" })).toBe(
      "GeneratePromptAgent",
    );
    expect(
      agentLogName({
        category: "agent",
        component: "generate_prompt",
        attributes: { agentDisplay: "GeneratePromptAgent" },
      }),
    ).toBe("GeneratePromptAgent");
    expect(agentLogName({ category: "models", component: "catalog" })).toBe("");
  });

  it("drops ring events for a trace file", () => {
    expect(eventBelongsToLogFile("agent", "promptlab-trace.log.2026-10-23")).toBe(false);
  });
});
