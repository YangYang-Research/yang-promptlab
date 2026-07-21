export type RuntimeInferenceSite = {
  id: string;
  title: string;
  location: string;
  description: string;
};

/** Workflows that invoke Yazg (AI Runtime) for inference (not display-only or config screens). */
export const RUNTIME_INFERENCE_SITES: RuntimeInferenceSite[] = [
  {
    id: "auth-verify",
    title: "Endpoint verification",
    location: "Scan wizard · Step 3 · AnalyzeEndpointAgent",
    description:
      "Yazg ReActs then delegates to AnalyzeEndpointAgent: after the HTTP probe succeeds, the agent classifies whether the response is from a live AI API.",
  },
  {
    id: "attack-plan",
    title: "Attack plan generation",
    location: "Scan wizard · Step 4 · AttackPlanAgent",
    description:
      "Yazg ReActs then delegates to AttackPlanAgent: reads the verified target profile and selects categories, techniques, execution strategy, and payload policy. Re-plan uses the same path.",
  },
  {
    id: "payload-gen",
    title: "Payload generation",
    location: "Scan execution · Generate phase",
    description:
      "When the attack plan enables local LLM payload mode, Yazg generates context-aware payload variants before each attack batch.",
  },
  {
    id: "judge",
    title: "Response judging",
    location: "Scan execution · Judge phase",
    description:
      "Yazg scores each attack response, assigns severity, and writes finding records with confidence and reasoning.",
  },
  {
    id: "recommendations",
    title: "Remediation recommendations",
    location: "Scan wizard · Step 6 · Scan details",
    description:
      "Yazg summarizes scan findings and produces prioritized fix guidance. Falls back to rule-based text if Yazg is unavailable.",
  },
  {
    id: "project-summary",
    title: "Project summary",
    location: "Project details · Summary",
    description:
      "Yazg summarizes overall project posture across targets, scans, and findings. The result is persisted on the project and reused until regenerated.",
  },
  {
    id: "attack-factory-prompt",
    title: "Attack Factory prompt generation",
    location: "Advanced · Attack Factory",
    description:
      "Yazg invents a novel adversarial probe for the selected technique from its metadata and current factory prompt. Review and save before it replaces the catalog entry.",
  },
];
