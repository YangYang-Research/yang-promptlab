import { invokeCommand } from "./invoke";

export type ScenarioAgentPlanRequest = {
  scenario: string;
  goal: string;
  classId?: string | null;
  categoryHint?: string | null;
  mutatorHint?: string[];
  successKeywordHint?: string[];
  seedHint?: string | null;
};

export type ScenarioAgentPlanDto = {
  seedPrompt: string;
  categoryId: string;
  recommendedMutators: string[];
  successKeywords: string[];
  attackHints: string[];
  canary: string;
};

export type ScenarioAgentVariantDto = {
  mutatorId: string;
  content: string;
};

export type ScenarioAgentExpandDto = {
  variants: ScenarioAgentVariantDto[];
};

export type ScenarioAgentFireRequest = {
  url?: string | null;
  targetId?: string | null;
  bodyTemplate?: string | null;
  method?: string | null;
  payload: string;
  timeoutMs?: number | null;
  successKeywords?: string[];
};

export type ScenarioAgentFireDto = {
  status: number;
  body: string;
  durationMs: number;
  goalHit: boolean;
  matchedKeywords: string[];
};

export type ScenarioAgentFollowupRequest = {
  scenario: string;
  goal: string;
  lastPrompt: string;
  lastResponse: string;
  transcript?: string;
  successKeywords?: string[];
};

export type ScenarioAgentFollowupDto = {
  prompts: string[];
  rationale: string;
};

export function scenarioAgentPlan(
  request: ScenarioAgentPlanRequest,
): Promise<ScenarioAgentPlanDto> {
  return invokeCommand<ScenarioAgentPlanDto>("scenario_agent_plan", { request });
}

export function scenarioAgentExpand(
  prompt: string,
  mutatorIds: string[],
): Promise<ScenarioAgentExpandDto> {
  return invokeCommand<ScenarioAgentExpandDto>("scenario_agent_expand", {
    request: { prompt, mutatorIds },
  });
}

export function scenarioAgentFire(
  request: ScenarioAgentFireRequest,
): Promise<ScenarioAgentFireDto> {
  return invokeCommand<ScenarioAgentFireDto>("scenario_agent_fire", { request });
}

export function scenarioAgentFollowup(
  request: ScenarioAgentFollowupRequest,
): Promise<ScenarioAgentFollowupDto> {
  return invokeCommand<ScenarioAgentFollowupDto>("scenario_agent_followup", {
    request,
  });
}
