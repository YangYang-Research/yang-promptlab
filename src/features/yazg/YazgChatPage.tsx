import { useEffect, useMemo, useRef, useState } from "react";

import {
  Button,
  Card,
  PageHeader,
  PageLoadingSkeleton,
} from "@/shared/components";
import { toAppError } from "@/shared/errors";
import { useAiInferenceRoute } from "@/shared/hooks/useAiInferenceRoute";
import { healthCheck } from "@/shared/ipc";
import {
  yazgChat,
  yazgGenerateChatTitle,
  type YazgAgentEventDto,
  type YazgIntent,
} from "@/shared/ipc/yazg";
import { isYazgAgentLive, assertYazgAgentLive } from "@/shared/runtime/yazgAgentLive";
import { useToast } from "@/shared/notifications";

type ChatMessage = {
  id: string;
  role: "user" | "yazg";
  text: string;
  events?: YazgAgentEventDto[];
  intent?: string;
  at: number;
};

type ChatThread = {
  id: string;
  title: string;
  createdAt: number;
  updatedAt: number;
  messages: ChatMessage[];
};

type ChatStore = {
  threads: ChatThread[];
  activeThreadId: string;
};

const HISTORY_STORAGE_KEY = "yazg-chat-threads-v2";

function newId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function welcomeMessage(): ChatMessage {
  return {
    id: newId(),
    role: "yazg",
    text: "I am Yazg, your AI Assistant. Ask me how to work across the app.",
    events: [{ agent: "yazg", kind: "info", message: "Supervisor ready" }],
    at: Date.now(),
  };
}

function createThread(): ChatThread {
  const now = Date.now();
  return {
    id: newId(),
    title: "New chat",
    createdAt: now,
    updatedAt: now,
    messages: [welcomeMessage()],
  };
}

function createStore(): ChatStore {
  const thread = createThread();
  return { threads: [thread], activeThreadId: thread.id };
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

function previewText(text: string, max = 72): string {
  const trimmed = text.trim().replace(/\s+/g, " ");
  if (trimmed.length <= max) return trimmed;
  return `${trimmed.slice(0, max - 1)}…`;
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
            at: typeof msg.at === "number" ? msg.at : Date.now(),
          })),
        }))
      : [];
    if (threads.length === 0) return createStore();
    const activeThreadId =
      typeof parsed.activeThreadId === "string" &&
      threads.some((thread) => thread.id === parsed.activeThreadId)
        ? parsed.activeThreadId
        : threads[0].id;
    return { threads, activeThreadId };
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

export function YazgChatPage() {
  const [backendConnected, setBackendConnected] = useState(false);
  const { configuration, loading: configLoading } = useAiInferenceRoute({
    enabled: backendConnected,
  });
  const { notify } = useToast();
  const live = isYazgAgentLive(configuration);

  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [store, setStore] = useState<ChatStore>(() => loadStore());
  const bottomRef = useRef<HTMLDivElement | null>(null);

  const activeThread = useMemo(() => {
    return (
      store.threads.find((thread) => thread.id === store.activeThreadId) ??
      store.threads[0] ??
      null
    );
  }, [store]);

  const historyThreads = useMemo(() => {
    return store.threads
      .slice()
      .sort((a, b) => b.updatedAt - a.updatedAt);
  }, [store.threads]);

  useEffect(() => {
    void healthCheck()
      .then(() => setBackendConnected(true))
      .catch(() => setBackendConnected(false));
  }, []);

  useEffect(() => {
    persistStore(store);
  }, [store]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [activeThread?.id, activeThread?.messages.length, busy]);

  const updateActiveThread = (
    updater: (thread: ChatThread) => ChatThread,
  ) => {
    setStore((prev) => {
      const current =
        prev.threads.find((thread) => thread.id === prev.activeThreadId) ??
        prev.threads[0];
      if (!current) return prev;
      const nextThread = updater(current);
      return {
        activeThreadId: nextThread.id,
        threads: prev.threads.map((thread) =>
          thread.id === current.id ? nextThread : thread,
        ),
      };
    });
  };

  const startNewChat = () => {
    if (busy) return;
    const thread = createThread();
    setStore((prev) => ({
      activeThreadId: thread.id,
      threads: [thread, ...prev.threads],
    }));
    setDraft("");
  };

  const selectThread = (threadId: string) => {
    if (busy || threadId === store.activeThreadId) return;
    setStore((prev) => ({ ...prev, activeThreadId: threadId }));
    setDraft("");
  };

  const clearHistory = () => {
    if (busy) return;
    const thread = createThread();
    setStore({ threads: [thread], activeThreadId: thread.id });
    setDraft("");
  };

  const send = async (message: string, intent: YazgIntent = "auto") => {
    const trimmed = message.trim();
    if (!trimmed || busy || !activeThread) return;

    const threadId = activeThread.id;
    const yazg = await assertYazgAgentLive(backendConnected);
    if (!yazg.live) {
      notify(yazg.message, "error");
      updateActiveThread((thread) => {
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
      setDraft("");
      return;
    }

    setDraft("");
    const isFirstUserTurn = !activeThread.messages.some((msg) => msg.role === "user");
    updateActiveThread((thread) => {
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
    setBusy(true);

    try {
      const response = await yazgChat({
        message: trimmed,
        targetId: null,
        sessionId: `yazg-chat:${threadId}`,
        intent,
      });
      updateActiveThread((thread) => {
        if (thread.id !== threadId) return thread;
        return {
          ...thread,
          updatedAt: Date.now(),
          messages: [
            ...thread.messages,
            {
              id: newId(),
              role: "yazg",
              text: response.reply,
              events: response.events,
              intent: response.intent,
              at: Date.now(),
            },
          ],
        };
      });

      if (isFirstUserTurn) {
        void generateConversationTitle(trimmed, response.reply).then((title) => {
          setStore((prev) => ({
            ...prev,
            threads: prev.threads.map((thread) =>
              thread.id === threadId
                ? { ...thread, title, updatedAt: Date.now() }
                : thread,
            ),
          }));
        });
      }
    } catch (err) {
      const messageText = toAppError(err).message;
      notify(messageText, "error");
      updateActiveThread((thread) => {
        if (thread.id !== threadId) return thread;
        return {
          ...thread,
          updatedAt: Date.now(),
          messages: [
            ...thread.messages,
            {
              id: newId(),
              role: "yazg",
              text: `I could not complete that request.\n\n${messageText}`,
              events: [{ agent: "yazg", kind: "failed", message: messageText }],
              at: Date.now(),
            },
          ],
        };
      });
      if (isFirstUserTurn) {
        void generateConversationTitle(trimmed, null).then((title) => {
          setStore((prev) => ({
            ...prev,
            threads: prev.threads.map((thread) =>
              thread.id === threadId
                ? { ...thread, title, updatedAt: Date.now() }
                : thread,
            ),
          }));
        });
      }
    } finally {
      setBusy(false);
    }
  };

  if (configLoading && (!activeThread || activeThread.messages.length <= 1)) {
    return <PageLoadingSkeleton />;
  }

  return (
    <div className="yazg-chat-page">
      <PageHeader
        title="Yazg"
        description="AI Assistant that helps you work across the app."
        actions={
          <div className="yazg-chat-page__header-meta">
            <span
              className={`yazg-chat-page__live ${live ? "yazg-chat-page__live--on" : ""}`}
            >
              {live ? "Agent live" : "Offline"}
            </span>
          </div>
        }
      />

      {!backendConnected ? (
        <Card className="detail-section">
          <p className="text-muted">Connect the Tauri backend to chat with Yazg.</p>
        </Card>
      ) : null}

      <div className="yazg-chat-page__layout">
        <Card className="detail-section yazg-chat-page__sidebar">
          <div className="yazg-chat-page__history-header">
            <h2 className="detail-section__title">History</h2>
            <div className="yazg-chat-page__history-actions">
              <Button variant="secondary" size="sm" disabled={busy} onClick={startNewChat}>
                New chat
              </Button>
              <Button
                variant="ghost"
                size="sm"
                disabled={busy || historyThreads.length === 0}
                onClick={clearHistory}
              >
                Clear
              </Button>
            </div>
          </div>

          {historyThreads.length === 0 ? (
            <p className="text-muted text-sm">No conversations yet.</p>
          ) : (
            <ul className="yazg-chat-page__history-list" aria-label="Conversation history">
              {historyThreads.map((thread) => {
                const lastUser = [...thread.messages]
                  .reverse()
                  .find((msg) => msg.role === "user");
                return (
                  <li key={thread.id}>
                    <button
                      type="button"
                      className={`yazg-chat-page__history-item${
                        store.activeThreadId === thread.id
                          ? " yazg-chat-page__history-item--active"
                          : ""
                      }`}
                      onClick={() => selectThread(thread.id)}
                    >
                      <span className="yazg-chat-page__history-preview">
                        {previewText(thread.title)}
                      </span>
                      <span className="yazg-chat-page__history-time">
                        {lastUser
                          ? new Date(thread.updatedAt).toLocaleString()
                          : "Just started"}
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}

          <p className="detail-section__hint">
            Each history item is one conversation. Use New chat to start another.
          </p>
        </Card>

        <Card className="detail-section yazg-chat-page__thread">
          <div className="yazg-chat-page__messages" role="log" aria-live="polite">
            {(activeThread?.messages ?? []).map((msg) => (
              <article
                key={msg.id}
                className={`yazg-chat-bubble yazg-chat-bubble--${msg.role}`}
              >
                <header className="yazg-chat-bubble__meta">
                  {msg.role === "yazg" ? "Yazg" : "You"}
                  {msg.intent ? (
                    <span className="yazg-chat-bubble__intent">{msg.intent}</span>
                  ) : null}
                </header>
                <p className="yazg-chat-bubble__text">{msg.text}</p>
                {msg.events && msg.events.length > 0 ? (
                  <ul className="yazg-chat-events">
                    {msg.events.map((event, index) => (
                      <li key={`${msg.id}-${index}`} className="yazg-chat-events__item">
                        <span className="yazg-chat-events__agent">
                          {event.agent}
                        </span>
                        <span className="yazg-chat-events__kind">{event.kind}</span>
                        <span className="yazg-chat-events__msg">{event.message}</span>
                      </li>
                    ))}
                  </ul>
                ) : null}
              </article>
            ))}
            {busy ? (
              <p className="yazg-chat-page__typing text-muted text-sm">Yazg is working…</p>
            ) : null}
            <div ref={bottomRef} />
          </div>

          <form
            className="yazg-chat-page__composer"
            onSubmit={(event) => {
              event.preventDefault();
              void send(draft, "auto");
            }}
          >
            <textarea
              className="yazg-chat-page__input"
              rows={2}
              placeholder="Message Yazg…"
              value={draft}
              disabled={!backendConnected || busy}
              onChange={(event) => setDraft(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !event.shiftKey) {
                  event.preventDefault();
                  void send(draft, "auto");
                }
              }}
            />
            <Button
              type="submit"
              variant="primary"
              disabled={!backendConnected || busy || !draft.trim()}
            >
              Send
            </Button>
          </form>
        </Card>
      </div>
    </div>
  );
}
