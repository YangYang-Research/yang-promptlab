import { invokeCommand } from "./invoke";

export type YazgIntent =
  | "auto"
  | "chat"
  | "analyze_endpoint"
  | "verify"
  | "attack_plan"
  | "plan"
  | "generate_prompt"
  | "create_project"
  | "list_workspace";

export type YazgAgentEventDto = {
  agent: string;
  kind: string;
  message: string;
};

export type YazgChatRequest = {
  message: string;
  targetId?: string | null;
  /** Stable conversation session for STM (one thread = one session). */
  sessionId?: string | null;
  intent?: YazgIntent | null;
};

export type YazgCreatedProjectDto = {
  id: string;
  name: string;
  description?: string | null;
};

export type YazgChatResponse = {
  reply: string;
  intent: string;
  /** Actual tool that produced the reply; `intent` is a coarse category. */
  action?: string | null;
  events: YazgAgentEventDto[];
  rawOutput?: unknown;
  verified?: boolean | null;
  planSummary?: string | null;
  createdProject?: YazgCreatedProjectDto | null;
  traceId?: string | null;
};

export type YazgGenerateChatTitleRequest = {
  message: string;
  reply?: string | null;
};

export type YazgGenerateChatTitleResponse = {
  title: string;
};

export function yazgChat(request: YazgChatRequest): Promise<YazgChatResponse> {
  return invokeCommand<YazgChatResponse>("yazg_chat", { request });
}

export function yazgGenerateChatTitle(
  request: YazgGenerateChatTitleRequest,
): Promise<YazgGenerateChatTitleResponse> {
  return invokeCommand<YazgGenerateChatTitleResponse>("yazg_generate_chat_title", {
    request,
  });
}
