export type RuntimeInferenceSite = {
  id: string;
  title: string;
  location: string;
  description: string;
};

/** Features that call an agent for inference. */
export const RUNTIME_INFERENCE_SITES: RuntimeInferenceSite[] = [
  {
    id: "auth-verify",
    title: "Endpoint verification",
    location: "Scan wizard · Step 3 · ANALYZE_ENDPOINT_AGENT",
    description:
      "AnalyzeEndpointAgent decides whether the probed target is a live AI API.",
  },
  {
    id: "attack-plan",
    title: "Attack plan generation",
    location: "Scan wizard · Step 4 · ATTACK_PLAN_AGENT",
    description:
      "AttackPlanAgent produces the categories, techniques, execution mode, and payload policy for a verified target.",
  },
  {
    id: "payload-gen",
    title: "Payload generation",
    location: "Scan execution · Generate phase",
    description:
      "Payload variants for the attack plan.",
  },
  {
    id: "agentic-execution",
    title: "Agentic scan execution",
    location: "Scan execution · Agentic · AGENTIC_ATTACK_EXECUTION_AGENT",
    description:
      "AgenticAttackExecutionAgent pursues the attack plan across multiple attempts.",
  },
  {
    id: "sequential-execution",
    title: "Sequential scan execution",
    location: "Scan execution · Sequential · SEQUENTIAL_ATTACK_EXECUTION_AGENT",
    description:
      "SequentialAttackExecutionAgent pursues the attack plan one category at a time.",
  },
  {
    id: "judge",
    title: "Response judging",
    location: "Scan execution · Judge phase · JUDGE_COORDINATOR_AGENT",
    description:
      "JudgeCoordinatorAgent decides whether a target reply is a finding.",
  },
  {
    id: "recommendations",
    title: "Remediation recommendations",
    location: "Scan wizard · Step 6 · Scan details · RECOMMEND_AGENT",
    description:
      "RecommendAgent writes prioritized fixes for the findings in a scan.",
  },
  {
    id: "project-summary",
    title: "Project summary",
    location: "Project details · Summary · SUMMARY_AGENT",
    description:
      "SummaryAgent writes the security posture of a project.",
  },
  {
    id: "attack-factory-prompt",
    title: "Attack Factory prompt generation",
    location: "Advanced · Attack Factory · GENERATE_PROMPT_AGENT",
    description:
      "GeneratePromptAgent writes a new probe for one attack technique.",
  },
  {
    id: "jailbreak-agent",
    title: "Jailbreak",
    location: "Advanced · Prompt Lab · Jailbreak · JAILBREAK_AGENT",
    description:
      "JailbreakAgent writes prompts for one goal on a guarded model.",
  },
];
