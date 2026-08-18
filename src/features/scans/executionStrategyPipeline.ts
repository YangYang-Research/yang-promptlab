import type { AttackPlanConfig } from "./attackPlan";
import type { ScanStatusDto } from "@/shared/ipc";

export type ExecutionPhaseId =
  | "preparing"
  | "generate"
  | "attack"
  | "judge"
  | "recover"
  | "reflection"
  | "adaptive"
  | "retry";

export type ExecutionStrategyStep = {
  id: ExecutionPhaseId | string;
  label: string;
  description: string;
};

export type ExecutionStepState = "pending" | "active" | "done" | "failed";

export type ExecutionTrailStep = {
  id: string;
  phase: string;
  label: string;
  state: ExecutionStepState;
};

const PHASE_META: Record<string, { label: string; description: string }> = {
  preparing: {
    label: "Preparing",
    description: "Warm up the attack monitor before the first probe",
  },
  generate: {
    label: "Generate · Payload",
    description: "Build or load payloads for the active category",
  },
  attack: {
    label: "Attack",
    description: "Send payloads to the target API and capture responses",
  },
  judge: {
    label: "Judge",
    description: "Score each response and record findings",
  },
  recover: {
    label: "Recover",
    description: "Adjust endpoint pacing after transport or health failures",
  },
  reflection: {
    label: "Reflection",
    description: "Review outcomes and decide whether another attempt is needed",
  },
  adaptive: {
    label: "Adaptive plan",
    description: "Replan techniques and escalate payload strategy before the next attempt",
  },
  retry: {
    label: "Retry",
    description: "Loop with escalated payloads for another attempt",
  },
};

/** Static plan outline — used only before a live trail exists (review / idle). */
export function executionStrategySteps(
  plan: Pick<
    AttackPlanConfig,
    "executionStrategy" | "reflectionEnabled" | "adaptivePlanning" | "maxAttempts"
  >,
): ExecutionStrategyStep[] {
  if (plan.executionStrategy === "sequential") {
    return [
      {
        id: "preparing",
        label: PHASE_META.preparing.label,
        description: PHASE_META.preparing.description,
      },
      {
        id: "generate",
        label: PHASE_META.generate.label,
        description: PHASE_META.generate.description,
      },
      {
        id: "attack",
        label: PHASE_META.attack.label,
        description: PHASE_META.attack.description,
      },
      {
        id: "judge",
        label: PHASE_META.judge.label,
        description: PHASE_META.judge.description,
      },
    ];
  }

  const steps: ExecutionStrategyStep[] = [
    {
      id: "preparing",
      label: PHASE_META.preparing.label,
      description: PHASE_META.preparing.description,
    },
    {
      id: "generate",
      label: PHASE_META.generate.label,
      description: PHASE_META.generate.description,
    },
    {
      id: "attack",
      label: PHASE_META.attack.label,
      description: PHASE_META.attack.description,
    },
    {
      id: "judge",
      label: PHASE_META.judge.label,
      description: PHASE_META.judge.description,
    },
  ];

  if (plan.reflectionEnabled) {
    steps.push({
      id: "reflection",
      label: PHASE_META.reflection.label,
      description: PHASE_META.reflection.description,
    });
  }

  if (plan.adaptivePlanning) {
    steps.push({
      id: "adaptive",
      label: PHASE_META.adaptive.label,
      description: PHASE_META.adaptive.description,
    });
  }

  steps.push({
    id: "retry",
    label: PHASE_META.retry.label,
    description: `Loop with escalated payloads (up to ${plan.maxAttempts} attempts per category)`,
  });

  return steps;
}

export function executionStrategyTitle(
  plan: Pick<AttackPlanConfig, "executionStrategy">,
): string {
  return plan.executionStrategy === "agentic" ? "Agentic execution" : "Sequential execution";
}

export function phaseLabel(phase: string): string {
  const key = phase.trim().toLowerCase();
  return PHASE_META[key]?.label ?? key.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
}

/** Trail entries may be `attack` or `attack|Prompt Injection`. */
export function parsePhaseTrailEntry(raw: string): { phase: string; category: string | null } {
  const trimmed = raw.trim();
  if (!trimmed) return { phase: "", category: null };
  const sep = trimmed.indexOf("|");
  if (sep < 0) {
    return { phase: trimmed.toLowerCase(), category: null };
  }
  const phase = trimmed.slice(0, sep).trim().toLowerCase();
  const category = trimmed.slice(sep + 1).trim() || null;
  return { phase, category };
}

export function phaseTrailStepLabel(phase: string, category: string | null): string {
  const base = phaseLabel(phase);
  if (category && (phase === "attack" || phase === "judge")) {
    return `${base} · ${category}`;
  }
  return base;
}

function normalizePhase(phase: string | null | undefined): string | null {
  if (!phase) return null;
  const value = phase.trim().toLowerCase();
  return value || null;
}

/** Merge backend trail with current_phase (covers poll gaps / older builds). */
export function resolvePhaseTrail(status: ScanStatusDto | null | undefined): string[] {
  if (!status) return [];
  const trail = [...(status.phase_trail ?? [])].map((phase) => phase.trim()).filter(Boolean);
  const current = normalizePhase(status.current_phase);
  if (!current) return trail;

  const last = trail.length > 0 ? parsePhaseTrailEntry(trail[trail.length - 1]) : null;
  if (last?.phase === current) {
    // Enrich a plain attack/judge entry with current category when trail lagged.
    if (
      !last.category &&
      (current === "attack" || current === "judge") &&
      status.current_test?.trim()
    ) {
      const enriched = `${current}|${status.current_test.trim()}`;
      return [...trail.slice(0, -1), enriched];
    }
    return trail;
  }

  // Preparing / Generate are scan-level stages — don't append a second node when
  // Retry or the next category briefly reports current_phase=generate.
  if (
    (current === "preparing" || current === "generate") &&
    trail.some((entry) => parsePhaseTrailEntry(entry).phase === current)
  ) {
    return trail;
  }

  if (
    (current === "attack" || current === "judge") &&
    status.current_test?.trim()
  ) {
    trail.push(`${current}|${status.current_test.trim()}`);
  } else {
    trail.push(current);
  }
  return trail;
}

/**
 * Live pipeline: only stages that have actually run, in order.
 * Example: Generate · Payload → Attack · Jailbreak → Recover → Attack · Jailbreak → Judge · Jailbreak
 */
export function resolveExecutionTrail(
  status: ScanStatusDto | null | undefined,
): ExecutionTrailStep[] {
  const phases = resolvePhaseTrail(status);
  if (phases.length === 0) return [];

  const failed = status?.status === "failed" || status?.status === "stopped";
  const terminal = ["completed", "failed", "cancelled", "stopped", "error"].includes(
    status?.status ?? "",
  );
  const lastIndex = phases.length - 1;

  return phases.map((raw, index) => {
    const { phase, category } = parsePhaseTrailEntry(raw);
    let state: ExecutionStepState = "done";
    if (!terminal && index === lastIndex) {
      state = failed ? "failed" : "active";
    } else if (terminal && failed && index === lastIndex) {
      state = "failed";
    }
    return {
      id: `${raw}-${index}`,
      phase,
      label: phaseTrailStepLabel(phase, category),
      state,
    };
  });
}

/** @deprecated Prefer resolveExecutionTrail for live scans. */
export function resolveExecutionStepStates(
  steps: ExecutionStrategyStep[],
  status: ScanStatusDto | null | undefined,
): ExecutionStepState[] {
  const trail = resolveExecutionTrail(status);
  if (trail.length > 0) {
    // Best-effort mapping onto the static outline for legacy callers.
    const latest = trail[trail.length - 1];
    const currentIndex = steps.findIndex((step) => step.id === latest.phase);
    if (currentIndex < 0) {
      return steps.map(() => "pending");
    }
    const terminal = ["completed", "failed", "cancelled", "stopped"].includes(
      status?.status ?? "",
    );
    if (terminal && status?.status === "completed") {
      return steps.map(() => "done");
    }
    return steps.map((_, index) => {
      if (index < currentIndex) return "done";
      if (index > currentIndex) return "pending";
      return latest.state;
    });
  }

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
