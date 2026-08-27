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

export type YazgHiltPendingActionDto = {
  id: string;
  tool: string;
  kind: "create" | "update" | "delete" | string;
  args: unknown;
  summary: string;
  /** Unix epoch ms when the pending action was created. */
  createdAtMs: number;
  /** Unix epoch ms when the pending action expires (auto-deny). */
  expiresAtMs: number;
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
  /** Mutating tool awaiting Approve / Deny (HILT). */
  pendingAction?: YazgHiltPendingActionDto | null;
  traceId?: string | null;
};

export type YazgGenerateChatTitleRequest = {
  message: string;
  reply?: string | null;
};

export type YazgGenerateChatTitleResponse = {
  title: string;
};

export type YazgResolveHiltRequest = {
  actionId: string;
  decision: "approve" | "deny" | "expire";
  sessionId?: string | null;
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

export function yazgResolveHilt(
  request: YazgResolveHiltRequest,
): Promise<YazgChatResponse> {
  return invokeCommand<YazgChatResponse>("yazg_resolve_hilt", { request });
}

export function yazgStop(): Promise<void> {
  return invokeCommand<void>("yazg_stop");
}

export type YazgChatStoreDto = {
  threads: unknown[];
  activeThreadId: string;
};

export function yazgChatThreadsGet(): Promise<YazgChatStoreDto | null> {
  return invokeCommand<YazgChatStoreDto | null>("yazg_chat_threads_get");
}

export function yazgChatThreadsSave(store: YazgChatStoreDto): Promise<void> {
  return invokeCommand<void>("yazg_chat_threads_save", { store });
}

/** Harness/LLM cooperative cancel from `yazg_stop`. */
export function isYazgCancelledError(error: unknown): boolean {
  const message =
    error && typeof error === "object" && "message" in error
      ? String((error as { message: unknown }).message)
      : String(error ?? "");
  const normalized = message.toLowerCase();
  return (
    normalized === "cancelled" ||
    normalized.includes("cancelled") ||
    normalized.includes("canceled")
  );
}
