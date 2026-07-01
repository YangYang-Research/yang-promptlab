import { describe, expect, it } from "vitest";

import {
  categoryIdFromCurrentTest,
  resolveAttackGraphStates,
} from "@/features/scans/attackGraphProgress";

describe("attack graph progress", () => {
  const categories = ["prompt_injection", "jailbreak", "system_prompt_extraction"] as const;

  it("maps current test label to category id", () => {
    expect(categoryIdFromCurrentTest("Prompt Injection")).toBe("prompt_injection");
    expect(categoryIdFromCurrentTest("Jailbreak")).toBe("jailbreak");
  });

  it("marks completed and active nodes during a run", () => {
    const states = resolveAttackGraphStates([...categories], {
      scan_id: "scan-1",
      status: "running",
      progress_percent: 33,
      completed: 1,
      total: 3,
      findings_count: 0,
      current_endpoint: "https://api.example.com/v1/chat",
      current_test: "Jailbreak",
      started_at: null,
      agent_mode: false,
      current_phase: null,
      current_attempt: null,
      current_retry: null,
    });

    expect(states.get("prompt_injection")).toBe("done");
    expect(states.get("jailbreak")).toBe("active");
    expect(states.get("system_prompt_extraction")).toBe("pending");
  });

  it("marks all nodes done when attack completes", () => {
    const states = resolveAttackGraphStates([...categories], {
      scan_id: "scan-1",
      status: "completed",
      progress_percent: 100,
      completed: 3,
      total: 3,
      findings_count: 1,
      current_endpoint: null,
      current_test: null,
      started_at: null,
      agent_mode: false,
      current_phase: null,
      current_attempt: null,
      current_retry: null,
    });

    expect(states.get("prompt_injection")).toBe("done");
    expect(states.get("jailbreak")).toBe("done");
    expect(states.get("system_prompt_extraction")).toBe("done");
  });
});
