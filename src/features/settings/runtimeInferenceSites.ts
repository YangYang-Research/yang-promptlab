export type RuntimeInferenceSite = {
  id: string;
  title: string;
  location: string;
  description: string;
};

/** Workflows that invoke AI Runtime for inference (not display-only or config screens). */
export const RUNTIME_INFERENCE_SITES: RuntimeInferenceSite[] = [
  {
    id: "auth-verify",
    title: "Endpoint verification",
    location: "Scan wizard · Step 3",
    description:
      "After the HTTP probe succeeds, AI Runtime classifies whether the response is from a live AI API.",
  },
  {
    id: "attack-plan",
    title: "Attack plan generation",
    location: "Scan wizard · Step 4",
    description:
      "AI Runtime reads the verified target profile and generates categories, execution strategy, and payload policy. Re-plan runs the same inference path.",
  },
  {
    id: "payload-gen",
    title: "Payload generation",
    location: "Scan execution · Generate phase",
    description:
      "When the attack plan enables local LLM payload mode, AI Runtime generates context-aware payload variants before each attack batch.",
  },
  {
    id: "judge",
    title: "Response judging",
    location: "Scan execution · Judge phase",
    description:
      "AI Runtime scores each attack response, assigns severity, and writes finding records with confidence and reasoning.",
  },
  {
    id: "recommendations",
    title: "Remediation recommendations",
    location: "Scan wizard · Step 6",
    description:
      "AI Runtime summarizes scan findings and produces prioritized fix guidance. Falls back to rule-based text if runtime is unavailable.",
  },
  {
    id: "attack-factory-prompt",
    title: "Attack Factory prompt generation",
    location: "Advanced · Attack Factory",
    description:
      "Generate new prompt invents a novel adversarial probe for the selected technique from its metadata and current factory prompt. Review and save before it replaces the catalog entry.",
  },
];
