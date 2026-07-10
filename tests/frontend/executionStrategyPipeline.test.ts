import { describe, expect, it } from "vitest";

import {
  executionStrategySteps,
  resolveExecutionStepStates,
} from "@/features/scans/executionStrategyPipeline";

describe("executionStrategySteps", () => {
  it("returns sequential generate → attack → judge", () => {
    const steps = executionStrategySteps({
      executionStrategy: "sequential",
      reflectionEnabled: false,
      adaptivePlanning: false,
      maxAttempts: 1,
    });
    expect(steps.map((step) => step.id)).toEqual(["generate", "attack", "judge"]);
  });

  it("includes reflection and retry for agentic plans", () => {
    const steps = executionStrategySteps({
      executionStrategy: "agentic",
      reflectionEnabled: true,
      adaptivePlanning: false,
      maxAttempts: 3,
    });
    expect(steps.map((step) => step.id)).toEqual([
      "generate",
      "attack",
      "judge",
      "reflection",
      "retry",
    ]);
  });

  it("includes adaptive plan between reflection and retry", () => {
    const steps = executionStrategySteps({
      executionStrategy: "agentic",
      reflectionEnabled: true,
      adaptivePlanning: true,
      maxAttempts: 3,
    });
    expect(steps.map((step) => step.id)).toEqual([
      "generate",
      "attack",
      "judge",
      "reflection",
      "adaptive",
      "retry",
    ]);
  });
});

describe("resolveExecutionStepStates", () => {
  const sequential = executionStrategySteps({
    executionStrategy: "sequential",
    reflectionEnabled: false,
    adaptivePlanning: false,
    maxAttempts: 1,
  });

  it("marks completed scans as done", () => {
    expect(
      resolveExecutionStepStates(sequential, {
        scan_id: "s1",
        status: "completed",
        progress_percent: 100,
        completed: 3,
        total: 3,
        findings_count: 1,
        current_endpoint: null,
        current_test: null,
        started_at: null,
        agent_mode: false,
        current_phase: "judge",
        current_attempt: null,
        current_retry: null,
      }),
    ).toEqual(["done", "done", "done"]);
  });

  it("highlights the active phase while running", () => {
    expect(
      resolveExecutionStepStates(sequential, {
        scan_id: "s1",
        status: "running",
        progress_percent: 40,
        completed: 1,
        total: 3,
        findings_count: 0,
        current_endpoint: "https://api.example/v1/chat",
        current_test: "Prompt Injection",
        started_at: null,
        agent_mode: false,
        current_phase: "attack",
        current_attempt: null,
        current_retry: null,
      }),
    ).toEqual(["done", "active", "pending"]);
  });
});
