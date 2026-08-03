import { invokeCommand } from "./invoke";

export type AgentStmSessionDto = {
  sessionId: string;
  eventCount: number;
  firstAt?: string | null;
  lastAt?: string | null;
};

export type AgentStmEventDto = {
  id: string;
  agentId: string;
  role: string;
  memoryKey?: string | null;
  content: string;
  contentJson?: unknown;
  importance: number;
  createdAt?: string | null;
};

export type AgentLtmEntryDto = {
  id: string;
  agentId: string;
  scopeType: string;
  scopeId: string;
  memoryKey: string;
  content: string;
  importance: number;
};

export function listAgentStmSessions(input?: {
  prefix?: string | null;
  limit?: number | null;
}): Promise<AgentStmSessionDto[]> {
  return invokeCommand<AgentStmSessionDto[]>("agent_memory_list_sessions", {
    request: {
      prefix: input?.prefix ?? "yazg-chat:",
      limit: input?.limit ?? 100,
    },
  });
}

export function listAgentStmEvents(input: {
  sessionId: string;
  agentId?: string | null;
  limit?: number | null;
}): Promise<AgentStmEventDto[]> {
  return invokeCommand<AgentStmEventDto[]>("agent_memory_list_events", {
    request: {
      sessionId: input.sessionId,
      agentId: input.agentId ?? null,
      limit: input.limit ?? 500,
    },
  });
}

export function listAgentLtm(input: {
  scopeType: string;
  scopeId?: string | null;
  agentId?: string | null;
  limit?: number | null;
}): Promise<AgentLtmEntryDto[]> {
  return invokeCommand<AgentLtmEntryDto[]>("agent_memory_list_ltm", {
    request: {
      scopeType: input.scopeType,
      scopeId: input.scopeId ?? "",
      agentId: input.agentId ?? "yazg",
      limit: input.limit ?? 64,
    },
  });
}

export function deleteAgentStmSession(sessionId: string): Promise<{ deleted: number }> {
  return invokeCommand<{ deleted: number }>("agent_memory_delete_session", {
    request: { sessionId },
  });
}
