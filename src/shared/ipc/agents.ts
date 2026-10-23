import { invokeCommand } from "./invoke";

export type AgentSummaryDto = {
  id: string;
  displayName: string;
  description: string;
  promptCount: number;
  toolCount: number;
  attempt: number | null;
  temperature: number | null;
};

export type AgentPromptDto = {
  id: string;
  system: string;
};

export type AgentToolDto = {
  id: string;
  description: string;
};

export type AgentDetailDto = {
  id: string;
  displayName: string;
  description: string;
  prompts: AgentPromptDto[];
  tools: AgentToolDto[];
  attempt: number | null;
  attemptNote: string | null;
  temperature: number | null;
  temperatureNote: string | null;
};

export function listAgents(): Promise<AgentSummaryDto[]> {
  return invokeCommand<AgentSummaryDto[]>("agents_list");
}

export function getAgent(agentId: string): Promise<AgentDetailDto> {
  return invokeCommand<AgentDetailDto>("agents_get", { request: { agentId } });
}

export function setAgentAttempt(
  agentId: string,
  attempt: number,
): Promise<AgentDetailDto> {
  return invokeCommand<AgentDetailDto>("agents_set_attempt", {
    request: { agentId, attempt },
  });
}

export function setAgentTemperature(
  agentId: string,
  temperature: number | null,
): Promise<AgentDetailDto> {
  return invokeCommand<AgentDetailDto>("agents_set_temperature", {
    request: { agentId, temperature },
  });
}
