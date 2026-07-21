import { invokeCommand } from "./invoke";

export type YazgIntent =
  | "auto"
  | "chat"
  | "analyze_endpoint"
  | "verify"
  | "attack_plan"
  | "plan";

export type YazgAgentEventDto = {
  agent: string;
  kind: string;
  message: string;
};

export type YazgChatRequest = {
  message: string;
  targetId?: string | null;
  intent?: YazgIntent | null;
};

export type YazgChatResponse = {
  reply: string;
  intent: string;
  events: YazgAgentEventDto[];
  verified?: boolean | null;
  planSummary?: string | null;
};

export function yazgChat(request: YazgChatRequest): Promise<YazgChatResponse> {
  return invokeCommand<YazgChatResponse>("yazg_chat", { request });
}
