import { invokeCommand } from "./invoke";

export type AgentTraceSessionDto = {
  sessionId: string;
  traceCount: number;
  firstAt?: string | null;
  lastAt?: string | null;
};

export type AgentTraceSummaryDto = {
  id: string;
  experimentId: string;
  experimentName: string;
  name: string;
  sessionId?: string | null;
  status: string;
  startedAt: string;
  endedAt?: string | null;
  latencyMs?: number | null;
  spanCount: number;
  totalTokens?: number | null;
  tags: Record<string, string>;
};

export type AgentTraceSpanDto = {
  id: string;
  traceId: string;
  parentSpanId?: string | null;
  name: string;
  kind: string;
  status: string;
  startedAt: string;
  endedAt?: string | null;
  latencyMs?: number | null;
  inputs?: unknown;
  outputs?: unknown;
  metrics: Record<string, number>;
  attributes: Record<string, string>;
};

export type AgentTraceDetailDto = {
  trace: AgentTraceSummaryDto;
  spans: AgentTraceSpanDto[];
};

export function listAgentTraceSessions(input?: {
  experiment?: string | null;
  limit?: number | null;
}): Promise<AgentTraceSessionDto[]> {
  return invokeCommand<AgentTraceSessionDto[]>("agenttrace_list_sessions", {
    request: {
      experiment: input?.experiment ?? "yazg",
      limit: input?.limit ?? 100,
    },
  });
}

export function listAgentTraces(input?: {
  experiment?: string | null;
  sessionId?: string | null;
  limit?: number | null;
}): Promise<AgentTraceSummaryDto[]> {
  return invokeCommand<AgentTraceSummaryDto[]>("agenttrace_list_traces", {
    request: {
      experiment: input?.experiment ?? "yazg",
      sessionId: input?.sessionId ?? null,
      limit: input?.limit ?? 100,
    },
  });
}

export function getAgentTraceDetail(
  traceId: string,
): Promise<AgentTraceDetailDto | null> {
  return invokeCommand<AgentTraceDetailDto | null>("agenttrace_get_trace", {
    request: { traceId },
  });
}

export function deleteAgentTraceSession(
  sessionId: string,
): Promise<{ deleted: number }> {
  return invokeCommand<{ deleted: number }>("agenttrace_delete_session", {
    request: { sessionId },
  });
}
