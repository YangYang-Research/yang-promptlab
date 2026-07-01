import type { AttackPlanConfig } from "./attackPlan";
import type { ScanStatusDto } from "@/shared/ipc";

export type ExecutionPhaseId = "generate" | "attack" | "judge" | "reflection" | "retry";

export type ExecutionStrategyStep = {
  id: ExecutionPhaseId;
  label: string;
  description: string;
};

export type ExecutionStepState = "pending" | "active" | "done" | "failed";

export function executionStrategySteps(
  plan: Pick<AttackPlanConfig, "executionStrategy" | "reflectionEnabled" | "maxAttempts">,
): ExecutionStrategyStep[] {
  if (plan.executionStrategy === "sequential") {
    return [
      {
        id: "generate",
        label: "Generate",
        description: "Build payload variants for every enabled test case",
      },
      {
        id: "attack",
        label: "Attack",
        description: "Send payloads to the target API and capture responses",
      },
      {
        id: "judge",
        label: "Judge",
        description: "Score each response and record findings",
      },
    ];
  }

  const steps: ExecutionStrategyStep[] = [
    {
      id: "generate",
      label: "Generate",
      description: "Create or adapt payloads for the active category",
    },
    {
      id: "attack",
      label: "Attack",
      description: "Execute HTTP attempts against the target",
    },
    {
      id: "judge",
      label: "Judge",
      description: "Evaluate responses and score confidence",
    },
  ];

  if (plan.reflectionEnabled) {
    steps.push({
      id: "reflection",
      label: "Reflection",
      description: "Review outcomes and decide whether another attempt is needed",
    });
  }

  steps.push({
    id: "retry",
    label: "Retry",
    description: `Loop with escalated payloads (up to ${plan.maxAttempts} attempts per category)`,
  });

  return steps;
}

export function executionStrategyTitle(
  plan: Pick<AttackPlanConfig, "executionStrategy">,
): string {
  return plan.executionStrategy === "agentic" ? "Agentic execution" : "Sequential execution";
}

function normalizePhase(phase: string | null | undefined): ExecutionPhaseId | null {
  if (!phase) return null;
  const value = phase.trim().toLowerCase();
  if (
    value === "generate" ||
    value === "attack" ||
    value === "judge" ||
    value === "reflection" ||
    value === "retry"
  ) {
    return value;
  }
  return null;
}

export function resolveExecutionStepStates(
  steps: ExecutionStrategyStep[],
  status: ScanStatusDto | null | undefined,
): ExecutionStepState[] {
  const failed = status?.status === "failed" || status?.status === "stopped";
  const completed = status?.status === "completed";

  if (completed) {
    return steps.map(() => "done" as const);
  }

  const rawPhase = status?.current_phase?.trim().toLowerCase();
  if (rawPhase === "preparing") {
    return steps.map(() => "pending" as const);
  }

  const phase = normalizePhase(status?.current_phase);
  if (!phase) {
    if (status && ["running", "paused", "pending"].includes(status.status)) {
      return steps.map((_, index) => (index === 0 ? "active" : "pending"));
    }
    return steps.map(() => "pending");
  }

  const currentIndex = steps.findIndex((step) => step.id === phase);
  if (currentIndex < 0) {
    return steps.map(() => "pending");
  }

  return steps.map((_, index) => {
    if (index < currentIndex) return "done";
    if (index > currentIndex) return "pending";
    return failed ? "failed" : "active";
  });
}

export function executionPipelineLiveDetail(status: ScanStatusDto | null | undefined): string | null {
  if (!status) return null;

  if (status.current_phase === "preparing") {
    return "Preparing attack pipeline…";
  }

  const parts: string[] = [];
  if (status.current_test) {
    parts.push(status.current_test);
  }
  if (status.current_attempt != null) {
    parts.push(`attempt ${status.current_attempt}`);
  }
  if (status.current_retry != null && status.current_retry > 0) {
    parts.push(`retry ${status.current_retry}`);
  }

  return parts.length > 0 ? parts.join(" · ") : null;
}
