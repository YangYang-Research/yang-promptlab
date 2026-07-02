export type VerificationPipelineStepId = "auth" | "ai";

export type VerificationPipelineStep = {
  id: VerificationPipelineStepId;
  label: string;
  description: string;
};

export type VerificationStepState = "pending" | "active" | "done" | "failed";

export type VerificationPipelinePhase =
  | "idle"
  | "auth"
  | "ai"
  | "done"
  | "failed_auth"
  | "failed_ai";

export const VERIFICATION_PIPELINE_STEPS: VerificationPipelineStep[] = [
  {
    id: "auth",
    label: "Verify authentication",
    description: "Send a probe request and confirm credentials and connectivity",
  },
  {
    id: "ai",
    label: "Verify AI API Endpoint",
    description: "Use AI Runtime to confirm the response is from a generative AI system",
  },
];

export type VerificationSegmentState = "pending" | "active" | "done" | "failed";

export type VerificationSegmentProgress = {
  auth: VerificationSegmentState;
  ai: VerificationSegmentState;
};

export const AUTH_SEGMENT_LABEL = "Verify authentication";
export const AUTH_SEGMENT_ACTIVE_LABEL = "Checking authentication and endpoint connectivity…";

export function authSegmentLabel(state: VerificationSegmentState): string {
  return state === "active" ? AUTH_SEGMENT_ACTIVE_LABEL : AUTH_SEGMENT_LABEL;
}

export const AI_SEGMENT_LABEL = "Verify AI API Endpoint";
export const AI_SEGMENT_ACTIVE_LABEL = "Analyzing AI API Endpoint with AI Runtime…";

export function aiSegmentLabel(state: VerificationSegmentState): string {
  return state === "active" ? AI_SEGMENT_ACTIVE_LABEL : AI_SEGMENT_LABEL;
}

export function formatVerificationResultMessage(message: string): string {
  const trimmed = message.trim();
  if (!trimmed) {
    return "Result: Verification complete.";
  }
  if (/^result:/i.test(trimmed)) {
    return trimmed;
  }
  const body = trimmed.replace(/^verification succeeded\s*[—–-]\s*/i, "").trim();
  if (!body) {
    return "Result: Verification complete.";
  }
  return `Result: Verification complete - ${body}`;
}

export function formatVerificationFailureMessage(message: string): string {
  const trimmed = message.trim();
  if (!trimmed) {
    return "Result: Verification failed.";
  }
  if (/^result:/i.test(trimmed)) {
    return trimmed;
  }
  return `Result: Verification failed - ${trimmed}`;
}

export function resolveVerificationSegmentProgress(
  phase: VerificationPipelinePhase,
): VerificationSegmentProgress {
  if (phase === "idle") {
    return { auth: "pending", ai: "pending" };
  }
  if (phase === "auth") {
    return { auth: "active", ai: "pending" };
  }
  if (phase === "failed_auth") {
    return { auth: "failed", ai: "pending" };
  }
  if (phase === "ai") {
    return { auth: "done", ai: "active" };
  }
  if (phase === "done") {
    return { auth: "done", ai: "done" };
  }
  return { auth: "done", ai: "failed" };
}

export function resolveVerificationStepStates(
  phase: VerificationPipelinePhase,
): VerificationStepState[] {
  if (phase === "idle") {
    return ["pending", "pending"];
  }
  if (phase === "auth") {
    return ["active", "pending"];
  }
  if (phase === "ai") {
    return ["done", "active"];
  }
  if (phase === "done") {
    return ["done", "done"];
  }
  if (phase === "failed_auth") {
    return ["failed", "pending"];
  }
  return ["done", "failed"];
}

export function verificationPipelineLiveDetail(
  phase: VerificationPipelinePhase,
  resultMessage?: string | null,
): string | null {
  switch (phase) {
    case "auth":
      return null;
    case "ai":
      return null;
    case "failed_auth":
      return "Authentication or connectivity check failed.";
    case "failed_ai":
      return resultMessage
        ? formatVerificationFailureMessage(resultMessage)
        : "AI Runtime could not confirm an AI system response.";
    case "done":
      return resultMessage
        ? formatVerificationResultMessage(resultMessage)
        : "Result: Verification complete.";
    default:
      return null;
  }
}
