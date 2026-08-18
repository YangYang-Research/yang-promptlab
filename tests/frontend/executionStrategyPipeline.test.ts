import { describe, expect, it } from "vitest";

import {
  executionStrategySteps,
  resolveExecutionTrail,
  resolvePhaseTrail,
} from "@/features/scans/executionStrategyPipeline";
import type { ScanStatusDto } from "@/shared/ipc";

function status(partial: Partial<ScanStatusDto>): ScanStatusDto {
  return {
    scan_id: "s1",
    status: "running",
    progress_percent: 40,
    completed: 1,
    total: 4,
    findings_count: 0,
    current_endpoint: "https://api.example/v1/chat",
    current_test: "Jailbreak",
    started_at: null,
    agent_mode: false,
    current_phase: null,
    current_attempt: null,
    current_retry: null,
    phase_trail: [],
    ...partial,
  };
}

describe("executionStrategySteps", () => {
  it("returns sequential preparing → generate → attack → judge outline", () => {
    const steps = executionStrategySteps({
      executionStrategy: "sequential",
      reflectionEnabled: false,
      adaptivePlanning: false,
      maxAttempts: 1,
    });
    expect(steps.map((step) => step.id)).toEqual(["preparing", "generate", "attack", "judge"]);
  });

  it("includes reflection and retry for agentic plans", () => {
    const steps = executionStrategySteps({
      executionStrategy: "agentic",
      reflectionEnabled: true,
      adaptivePlanning: false,
      maxAttempts: 3,
    });
    expect(steps.map((step) => step.id)).toEqual([
      "preparing",
      "generate",
      "attack",
      "judge",
      "reflection",
      "retry",
    ]);
  });
});

describe("resolveExecutionTrail", () => {
  it("builds a live trail with repeated stages", () => {
    const trail = resolveExecutionTrail(
      status({
        phase_trail: ["generate", "attack", "recover", "attack", "judge"],
        current_phase: "judge",
        current_test: "Jailbreak",
      }),
    );
    expect(trail.map((step) => step.label)).toEqual([
      "Generate · Payload",
      "Attack",
      "Recover",
      "Attack",
      "Judge · Jailbreak",
    ]);
    expect(trail.map((step) => step.state)).toEqual([
      "done",
      "done",
      "done",
      "done",
      "active",
    ]);
  });

  it("labels attack and judge stages with category names from the trail", () => {
    const trail = resolveExecutionTrail(
      status({
        phase_trail: [
          "generate",
          "attack|Prompt Injection",
          "judge|Prompt Injection",
          "attack|Jailbreak",
          "judge|Jailbreak",
        ],
        current_phase: "judge",
        current_test: "Jailbreak",
      }),
    );
    expect(trail.map((step) => step.label)).toEqual([
      "Generate · Payload",
      "Attack · Prompt Injection",
      "Judge · Prompt Injection",
      "Attack · Jailbreak",
      "Judge · Jailbreak",
    ]);
  });

  it("appends current_phase when trail lags behind", () => {
    expect(
      resolvePhaseTrail(
        status({
          phase_trail: ["generate", "attack|Jailbreak"],
          current_phase: "recover",
        }),
      ),
    ).toEqual(["generate", "attack|Jailbreak", "recover"]);
  });

  it("keeps preparing as the first pipeline stage", () => {
    const trail = resolveExecutionTrail(
      status({
        phase_trail: ["preparing", "generate", "attack|Jailbreak"],
        current_phase: "attack",
        current_test: "Jailbreak",
      }),
    );
    expect(trail.map((step) => step.label)).toEqual([
      "Preparing",
      "Generate · Payload",
      "Attack · Jailbreak",
    ]);

    expect(
      resolvePhaseTrail(
        status({
          phase_trail: [],
          current_phase: "preparing",
          current_test: "loading attack monitor",
        }),
      ),
    ).toEqual(["preparing"]);
  });

  it("does not append a second Generate when the next category starts", () => {
    expect(
      resolvePhaseTrail(
        status({
          phase_trail: [
            "preparing",
            "generate",
            "attack|Prompt Injection",
            "judge|Prompt Injection",
          ],
          current_phase: "generate",
          current_test: "Jailbreak",
        }),
      ),
    ).toEqual([
      "preparing",
      "generate",
      "attack|Prompt Injection",
      "judge|Prompt Injection",
    ]);
  });

  it("marks the whole trail done when scan completed", () => {
    const trail = resolveExecutionTrail(
      status({
        status: "completed",
        phase_trail: ["generate", "attack|Jailbreak", "judge|Jailbreak"],
        current_phase: "judge",
        current_test: "Jailbreak",
      }),
    );
    expect(trail.every((step) => step.state === "done")).toBe(true);
  });
});
