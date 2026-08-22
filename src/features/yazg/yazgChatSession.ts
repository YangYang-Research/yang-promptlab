import { toAppError } from "@/shared/errors";
import { deleteAgentStmSession } from "@/shared/ipc/agentMemory";
import { deleteAgentTraceSession } from "@/shared/ipc/agentTrace";
import {
  yazgChat,
  yazgGenerateChatTitle,
  yazgResolveHilt,
  yazgStop,
  isYazgCancelledError,
  type YazgAgentEventDto,
  type YazgHiltPendingActionDto,
  type YazgIntent,
} from "@/shared/ipc/yazg";
import { assertYazgAgentLive } from "@/shared/runtime/yazgAgentLive";

export type ChatMessage = {
  id: string;
  role: "user" | "yazg";
  text: string;
  events?: YazgAgentEventDto[];
  rawOutput?: unknown;
  intent?: string;
  /** Actual tool behind the reply; preferred over the coarse `intent`. */
  action?: string;
  /** AgentTrace id for this Yazg reply (opens Trace detail). */
  traceId?: string;
  /** Mutating tool awaiting Approve / Deny (HILT). Cleared after resolve. */
  pendingAction?: YazgHiltPendingActionDto;
  /** Set after the user resolves a HILT card on this message. */
  hiltDecision?: "approve" | "deny" | "expire";
  at: number;
};

export type ChatThread = {
  id: string;
  title: string;
  createdAt: number;
  updatedAt: number;
  messages: ChatMessage[];
};

export type ChatStore = {
  threads: ChatThread[];
  activeThreadId: string;
};

export type YazgChatSessionState = {
  store: ChatStore;
  busy: boolean;
  stopping: boolean;
  /** Thread currently waiting on Yazg ReAct (survives route changes). */
  pendingThreadId: string | null;
};

type YazgChatHostHooks = {
  notify?: (message: string, level: "success" | "error" | "info") => void;
  refresh?: () => void;
};

const HISTORY_STORAGE_KEY = "yazg-chat-threads-v2";

function newId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function createThread(): ChatThread {
  const now = Date.now();
  return {
    id: newId(),
    title: "New chat",
    createdAt: now,
    updatedAt: now,
    messages: [],
  };
}

function createStore(): ChatStore {
  const thread = createThread();
  return { threads: [thread], activeThreadId: thread.id };
}

export function isBlankThread(thread: ChatThread): boolean {
  return !thread.messages.some((message) => message.role === "user");
}

function titleFromPrompt(text: string): string {
  const trimmed = text.trim().replace(/\s+/g, " ");
  if (!trimmed) return "New chat";
  const words = trimmed.split(" ").slice(0, 8);
  const title = words.join(" ");
  return trimmed.split(" ").length > 8 ? `${title}…` : title;
}

async function generateConversationTitle(
  message: string,
  reply?: string | null,
): Promise<string> {
  try {
    const result = await yazgGenerateChatTitle({
      message,
      reply: reply ?? null,
    });
    const title = result.title.trim();
    if (title) return title;
  } catch {
    // fall through
  }
  return titleFromPrompt(message);
}

function isChatMessage(value: unknown): value is ChatMessage {
  if (!value || typeof value !== "object") return false;
  const msg = value as Partial<ChatMessage>;
  return (
    typeof msg.id === "string" &&
    (msg.role === "user" || msg.role === "yazg") &&
    typeof msg.text === "string"
  );
}

function isChatThread(value: unknown): value is ChatThread {
  if (!value || typeof value !== "object") return false;
  const thread = value as Partial<ChatThread>;
  return (
    typeof thread.id === "string" &&
    typeof thread.title === "string" &&
    Array.isArray(thread.messages) &&
    thread.messages.every(isChatMessage)
  );
}

function loadStore(): ChatStore {
  try {
    const raw = localStorage.getItem(HISTORY_STORAGE_KEY);
    if (!raw) return createStore();
    const parsed = JSON.parse(raw) as Partial<ChatStore>;
    const threads = Array.isArray(parsed.threads)
      ? parsed.threads.filter(isChatThread).map((thread) => ({
          ...thread,
          createdAt: typeof thread.createdAt === "number" ? thread.createdAt : Date.now(),
          updatedAt: typeof thread.updatedAt === "number" ? thread.updatedAt : Date.now(),
          messages: thread.messages.map((msg) => ({
            ...msg,
            // HILT pending lives in an in-memory host store — drop stale cards.
            pendingAction: undefined,
            at: typeof msg.at === "number" ? msg.at : Date.now(),
          })),
        }))
      : [];
    if (threads.length === 0) return createStore();
    const blanks = threads.filter(isBlankThread);
    const conversations = threads.filter((thread) => !isBlankThread(thread));
    const blank =
      blanks.find((thread) => thread.id === parsed.activeThreadId) ??
      blanks.sort((a, b) => b.updatedAt - a.updatedAt)[0] ??
      null;
    const normalized = blank ? [blank, ...conversations] : conversations;
    if (normalized.length === 0) return createStore();
    const activeThreadId =
      typeof parsed.activeThreadId === "string" &&
      normalized.some((thread) => thread.id === parsed.activeThreadId)
        ? parsed.activeThreadId
        : normalized[0].id;
    return { threads: normalized, activeThreadId };
  } catch {
    return createStore();
  }
}

function persistStore(store: ChatStore) {
  try {
    const trimmed: ChatStore = {
      activeThreadId: store.activeThreadId,
      threads: store.threads.slice(0, 40).map((thread) => ({
        ...thread,
        messages: thread.messages.slice(-120),
      })),
    };
    localStorage.setItem(HISTORY_STORAGE_KEY, JSON.stringify(trimmed));
  } catch {
    // ignore quota / private mode
  }
}

type Listener = () => void;

let state: YazgChatSessionState = {
  store: loadStore(),
  busy: false,
  stopping: false,
  pendingThreadId: null,
};

const listeners = new Set<Listener>();
let hostHooks: YazgChatHostHooks = {};

function emit() {
  persistStore(state.store);
  for (const listener of listeners) {
    listener();
  }
}

function setState(next: YazgChatSessionState) {
  state = next;
  emit();
}

function patchStore(updater: (prev: ChatStore) => ChatStore) {
  setState({
    ...state,
    store: updater(state.store),
  });
}

function updateThread(
  threadId: string,
  updater: (thread: ChatThread) => ChatThread,
) {
  patchStore((prev) => {
    const current = prev.threads.find((thread) => thread.id === threadId);
    if (!current) return prev;
    const nextThread = updater(current);
    return {
      ...prev,
      threads: prev.threads.map((thread) =>
        thread.id === threadId ? nextThread : thread,
      ),
    };
  });
}

export function getYazgChatSessionSnapshot(): YazgChatSessionState {
  return state;
}

export function subscribeYazgChatSession(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Register UI side-effects while the chat page is mounted. */
export function setYazgChatHostHooks(hooks: YazgChatHostHooks) {
  hostHooks = hooks;
}

export function startNewYazgChat() {
  if (state.busy) return;
  patchStore((prev) => {
    const blanks = prev.threads.filter(isBlankThread);
    const conversations = prev.threads.filter((thread) => !isBlankThread(thread));
    if (blanks.length > 0) {
      const keep =
        blanks.find((thread) => thread.id === prev.activeThreadId) ??
        blanks.slice().sort((a, b) => b.updatedAt - a.updatedAt)[0];
      const refreshed = { ...keep, updatedAt: Date.now() };
      return {
        activeThreadId: refreshed.id,
        threads: [refreshed, ...conversations],
      };
    }
    const thread = createThread();
    return {
      activeThreadId: thread.id,
      threads: [thread, ...conversations],
    };
  });
}

export function selectYazgChatThread(threadId: string) {
  if (threadId === state.store.activeThreadId) return;
  if (!state.store.threads.some((thread) => thread.id === threadId)) return;
  patchStore((prev) => ({ ...prev, activeThreadId: threadId }));
}

export function renameYazgChatThread(threadId: string, title: string) {
  const nextTitle = title.trim().replace(/\s+/g, " ");
  if (!nextTitle) return;
  updateThread(threadId, (thread) => ({
    ...thread,
    title: nextTitle,
    updatedAt: Date.now(),
  }));
}

export function deleteYazgChatThread(threadId: string) {
  if (state.busy && state.pendingThreadId === threadId) return;
  patchStore((prev) => {
    const remaining = prev.threads.filter((thread) => thread.id !== threadId);
    if (remaining.length === 0) {
      const thread = createThread();
      return { threads: [thread], activeThreadId: thread.id };
    }
    const activeThreadId =
      prev.activeThreadId === threadId
        ? remaining.slice().sort((a, b) => b.updatedAt - a.updatedAt)[0].id
        : prev.activeThreadId;
    return { threads: remaining, activeThreadId };
  });
  // Agent Trace lists AgentTrace + STM by yazg-chat:<threadId>; clear both with the UI thread.
  const sessionId = `yazg-chat:${threadId}`;
  void deleteAgentStmSession(sessionId).catch(() => {
    /* best-effort */
  });
  void deleteAgentTraceSession(sessionId).catch(() => {
    /* best-effort — Trace will still purge orphans on refresh */
  });
}

/** Session ids still present in the Assistant chat store. */
export function yazgChatSessionIds(): Set<string> {
  return new Set(
    getYazgChatSessionSnapshot().store.threads.map(
      (thread) => `yazg-chat:${thread.id}`,
    ),
  );
}

export function yazgChatThreadTitle(sessionId: string): string | null {
  const prefix = "yazg-chat:";
  const threadId = sessionId.startsWith(prefix)
    ? sessionId.slice(prefix.length)
    : sessionId;
  const thread = getYazgChatSessionSnapshot().store.threads.find(
    (item) => item.id === threadId,
  );
  return thread?.title ?? null;
}

function appendYazgMessage(
  threadId: string,
  message: Omit<ChatMessage, "id" | "role" | "at"> & { text: string },
) {
  const now = Date.now();
  const yazgMessage: ChatMessage = {
    id: newId(),
    role: "yazg",
    text: message.text,
    events: message.events,
    rawOutput: message.rawOutput,
    intent: message.intent,
    action: message.action,
    traceId: message.traceId,
    pendingAction: message.pendingAction,
    hiltDecision: message.hiltDecision,
    at: now,
  };
  // Apply reply + clear busy in one snapshot so the UI never sticks on "working…".
  setState({
    busy: false,
    stopping: false,
    pendingThreadId: null,
    store: {
      ...state.store,
      threads: state.store.threads.map((thread) => {
        if (thread.id !== threadId) return thread;
        return {
          ...thread,
          updatedAt: now,
          messages: [...thread.messages, yazgMessage],
        };
      }),
    },
  });
}

/**
 * Send a user message. The IPC call keeps running even if the chat page unmounts;
 * results are written into the module store when they complete.
 */
export async function sendYazgChatMessage(options: {
  message: string;
  intent?: YazgIntent;
  backendConnected: boolean;
}): Promise<void> {
  const trimmed = options.message.trim();
  if (!trimmed || state.busy) return;

  const threadId = state.store.activeThreadId;
  const activeThread = state.store.threads.find((thread) => thread.id === threadId);
  if (!activeThread) return;

  const yazg = await assertYazgAgentLive(options.backendConnected);
  if (!yazg.live) {
    hostHooks.notify?.(yazg.message, "error");
    updateThread(threadId, (thread) => {
      const now = Date.now();
      const isFirstUser = !thread.messages.some((msg) => msg.role === "user");
      return {
        ...thread,
        title: isFirstUser ? titleFromPrompt(trimmed) : thread.title,
        updatedAt: now,
        messages: [
          ...thread.messages,
          { id: newId(), role: "user", text: trimmed, at: now },
          {
            id: newId(),
            role: "yazg",
            text: yazg.message,
            events: [{ agent: "yazg", kind: "failed", message: yazg.message }],
            at: now,
          },
        ],
      };
    });
    return;
  }

  const isFirstUserTurn = !activeThread.messages.some((msg) => msg.role === "user");
  updateThread(threadId, (thread) => {
    const now = Date.now();
    const isFirstUser = !thread.messages.some((msg) => msg.role === "user");
    return {
      ...thread,
      title: isFirstUser ? titleFromPrompt(trimmed) : thread.title,
      updatedAt: now,
      messages: [
        ...thread.messages,
        { id: newId(), role: "user", text: trimmed, at: now },
      ],
    };
  });

  setState({
    ...state,
    busy: true,
    stopping: false,
    pendingThreadId: threadId,
  });

  try {
    const response = await yazgChat({
      message: trimmed,
      targetId: null,
      sessionId: `yazg-chat:${threadId}`,
      intent: options.intent ?? "auto",
    });

    appendYazgMessage(threadId, {
      text: response.reply?.trim()
        ? response.reply
        : "Yazg finished without a reply. Try asking again.",
      events: response.events,
      rawOutput: response.rawOutput,
      intent: response.intent,
      action: response.action?.trim() || undefined,
      traceId: response.traceId?.trim() || undefined,
      pendingAction: response.pendingAction ?? undefined,
    });

    if (response.createdProject) {
      hostHooks.refresh?.();
      hostHooks.notify?.(
        `Created project "${response.createdProject.name}"`,
        "success",
      );
    }

    if (isFirstUserTurn) {
      void generateConversationTitle(trimmed, response.reply).then((title) => {
        updateThread(threadId, (thread) => ({
          ...thread,
          title,
          updatedAt: Date.now(),
        }));
      });
    }
  } catch (err) {
    if (isYazgCancelledError(err)) {
      hostHooks.notify?.("Stopped.", "info");
      appendYazgMessage(threadId, {
        text: "Stopped.",
        events: [{ agent: "yazg", kind: "cancelled", message: "Stopped." }],
      });
      return;
    }
    const messageText = toAppError(err).message;
    hostHooks.notify?.(messageText, "error");
    appendYazgMessage(threadId, {
      text: `I could not complete that request.\n\n${messageText}`,
      events: [{ agent: "yazg", kind: "failed", message: messageText }],
    });
    if (isFirstUserTurn) {
      void generateConversationTitle(trimmed, null).then((title) => {
        updateThread(threadId, (thread) => ({
          ...thread,
          title,
          updatedAt: Date.now(),
        }));
      });
    }
  } finally {
    // Safety net if try/catch returned before appendYazgMessage (should be rare).
    if (state.busy && state.pendingThreadId === threadId) {
      setState({
        ...state,
        busy: false,
        stopping: false,
        pendingThreadId: null,
      });
    }
  }
}

/** Cooperative cancel for the in-flight Assistant turn (`yazg_stop`). */
export async function stopYazgChat(): Promise<void> {
  if (!state.busy || state.stopping) return;
  setState({ ...state, stopping: true });
  try {
    await yazgStop();
  } catch (err) {
    setState({ ...state, stopping: false });
    hostHooks.notify?.(toAppError(err).message, "error");
  }
}

/**
 * Approve or deny a pending mutating tool (HILT). Clears the card on the
 * originating message and appends a follow-up Yazg reply.
 */
export async function resolveYazgHiltAction(options: {
  threadId: string;
  messageId: string;
  actionId: string;
  decision: "approve" | "deny" | "expire";
  backendConnected: boolean;
}): Promise<void> {
  if (state.busy) return;

  const yazg = await assertYazgAgentLive(options.backendConnected);
  if (!yazg.live) {
    hostHooks.notify?.(yazg.message, "error");
    return;
  }

  setState({
    ...state,
    busy: true,
    stopping: false,
    pendingThreadId: options.threadId,
  });

  try {
    const response = await yazgResolveHilt({
      actionId: options.actionId,
      decision: options.decision,
      sessionId: `yazg-chat:${options.threadId}`,
    });

    // Mark the originating card as resolved before appending the follow-up.
    updateThread(options.threadId, (thread) => ({
      ...thread,
      updatedAt: Date.now(),
      messages: thread.messages.map((msg) =>
        msg.id === options.messageId
          ? {
              ...msg,
              pendingAction: undefined,
              hiltDecision: options.decision,
            }
          : msg,
      ),
    }));

    appendYazgMessage(options.threadId, {
      text: response.reply?.trim()
        ? response.reply
        : options.decision === "approve"
          ? "Action approved."
          : "Action cancelled.",
      events: response.events,
      rawOutput: response.rawOutput,
      intent: response.intent,
      action: response.action?.trim() || undefined,
      traceId: response.traceId?.trim() || undefined,
      hiltDecision: options.decision,
    });

    if (response.createdProject) {
      hostHooks.refresh?.();
      hostHooks.notify?.(
        `Created project "${response.createdProject.name}"`,
        "success",
      );
    } else if (options.decision === "deny") {
      hostHooks.notify?.("Action cancelled", "success");
    } else if (options.decision === "expire") {
      hostHooks.notify?.("Approval expired (15 min)", "error");
    }
  } catch (err) {
    if (isYazgCancelledError(err)) {
      hostHooks.notify?.("Stopped.", "info");
      appendYazgMessage(options.threadId, {
        text: "Stopped.",
        events: [{ agent: "yazg", kind: "cancelled", message: "Stopped." }],
      });
      return;
    }
    const messageText = toAppError(err).message;
    hostHooks.notify?.(messageText, "error");
    setState({
      ...state,
      busy: false,
      stopping: false,
      pendingThreadId: null,
    });
  }
}
