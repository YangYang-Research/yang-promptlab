import { describe, expect, it } from "vitest";

import {
  attackGraphStateLabel,
  categoryIdFromCurrentTest,
  resolveAttackGraphStates,
} from "@/features/scans/attackGraphProgress";

describe("attack graph progress", () => {
  const categories = ["prompt_injection", "jailbreak", "system_prompt_extraction"] as const;

  it("maps current test label to category id", () => {
    expect(categoryIdFromCurrentTest("Prompt Injection")).toBe("prompt_injection");
    expect(categoryIdFromCurrentTest("Jailbreak")).toBe("jailbreak");
  });

  it("marks completed categories without using payload unit count", () => {
    const states = resolveAttackGraphStates([...categories], {
      scan_id: "scan-1",
      status: "running",
      progress_percent: 33,
      completed: 12,
      total: 30,
      categories_completed: 1,
      findings_count: 0,
      current_endpoint: "https://api.example.com/v1/chat",
      current_test: "Jailbreak",
      current_phase: "attack",
      started_at: null,
      agent_mode: false,
      current_attempt: null,
      current_retry: null,
    });

    expect(states.get("prompt_injection")).toBe("done");
    expect(states.get("jailbreak")).toBe("active");
    expect(states.get("system_prompt_extraction")).toBe("pending");
  });

  it("shows judging label for active judge phase", () => {
    const status = {
      scan_id: "scan-1",
      status: "running",
      progress_percent: 20,
      completed: 4,
      total: 20,
      categories_completed: 0,
      findings_count: 0,
      current_endpoint: "https://api.example.com/v1/chat",
      current_test: "Prompt Injection",
      current_phase: "judge",
      started_at: null,
      agent_mode: false,
      current_attempt: null,
      current_retry: null,
    };

    expect(attackGraphStateLabel("active", status, "prompt_injection")).toBe("Judging");
    expect(attackGraphStateLabel("active", status, "jailbreak")).toBe("Running");
  });

  it("marks all nodes done when every category completed successfully", () => {
    const states = resolveAttackGraphStates([...categories], {
      scan_id: "scan-1",
      status: "completed",
      progress_percent: 100,
      completed: 30,
      total: 30,
      categories_completed: 3,
      findings_count: 1,
      current_endpoint: null,
      current_test: null,
      current_phase: null,
      started_at: null,
      agent_mode: false,
      current_attempt: null,
      current_retry: null,
    });

    expect(states.get("prompt_injection")).toBe("done");
    expect(states.get("jailbreak")).toBe("done");
    expect(states.get("system_prompt_extraction")).toBe("done");
  });

  it("does not mark failed categories done when scan completed with findings", () => {
    const states = resolveAttackGraphStates([...categories], {
      scan_id: "scan-1",
      status: "completed",
      progress_percent: 100,
      completed: 20,
      total: 30,
      categories_completed: 2,
      categories_failed: ["jailbreak"],
      findings_count: 2,
      current_endpoint: null,
      current_test: null,
      current_phase: null,
      started_at: null,
      agent_mode: false,
      current_attempt: null,
      current_retry: null,
    });

    expect(states.get("prompt_injection")).toBe("done");
    expect(states.get("jailbreak")).toBe("failed");
    expect(states.get("system_prompt_extraction")).toBe("pending");
  });
});
