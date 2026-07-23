import { useEffect, useMemo, useRef, useState } from "react";

import {
  ActionsDropdown,
  Button,
  Card,
  IconButton,
  IconEdit,
  IconInfo,
  IconPlus,
  IconRobot,
  IconSend,
  IconTrash,
  Modal,
  PageHeader,
  PageLoadingSkeleton,
  SearchInput,
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

/** Empty draft: no user turns yet. */
function isBlankThread(thread: ChatThread): boolean {
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
  const [renameThreadId, setRenameThreadId] = useState<string | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [infoThreadId, setInfoThreadId] = useState<string | null>(null);
  const [historyQuery, setHistoryQuery] = useState("");
  const bottomRef = useRef<HTMLDivElement | null>(null);

  const activeThread = useMemo(() => {
    return (
      store.threads.find((thread) => thread.id === store.activeThreadId) ??
      store.threads[0] ??
      null
    );
  }, [store]);

  const historyThreads = useMemo(() => {
    const query = historyQuery.trim().toLowerCase();
    return store.threads
      .slice()
      .sort((a, b) => b.updatedAt - a.updatedAt)
      .filter((thread) => {
        if (!query) return true;
        if (thread.title.toLowerCase().includes(query)) return true;
        return thread.messages.some((message) =>
          message.text.toLowerCase().includes(query),
        );
      });
  }, [historyQuery, store.threads]);

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
    setStore((prev) => {
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
    setDraft("");
  };

  const selectThread = (threadId: string) => {
    if (busy || threadId === store.activeThreadId) return;
    setStore((prev) => ({ ...prev, activeThreadId: threadId }));
    setDraft("");
  };

  const openRename = (threadId: string) => {
    const thread = store.threads.find((item) => item.id === threadId);
    if (!thread) return;
    setRenameThreadId(threadId);
    setRenameDraft(thread.title);
  };

  const closeRename = () => {
    setRenameThreadId(null);
    setRenameDraft("");
  };

  const openInfo = (threadId: string) => {
    setInfoThreadId(threadId);
  };

  const closeInfo = () => {
    setInfoThreadId(null);
  };

  const infoThread = useMemo(
    () => store.threads.find((thread) => thread.id === infoThreadId) ?? null,
    [store.threads, infoThreadId],
  );

  const submitRename = () => {
    if (!renameThreadId) return;
    const nextTitle = renameDraft.trim().replace(/\s+/g, " ");
    if (!nextTitle) return;
    setStore((prev) => ({
      ...prev,
      threads: prev.threads.map((thread) =>
        thread.id === renameThreadId
          ? { ...thread, title: nextTitle, updatedAt: Date.now() }
          : thread,
      ),
    }));
    closeRename();
  };

  const deleteThread = (threadId: string) => {
    if (busy) return;
    setStore((prev) => {
      const remaining = prev.threads.filter((thread) => thread.id !== threadId);
      if (remaining.length === 0) {
        const thread = createThread();
        return { threads: [thread], activeThreadId: thread.id };
      }
      const activeThreadId =
        prev.activeThreadId === threadId
          ? remaining
              .slice()
              .sort((a, b) => b.updatedAt - a.updatedAt)[0].id
          : prev.activeThreadId;
      return { threads: remaining, activeThreadId };
    });
    if (renameThreadId === threadId) {
      closeRename();
    }
    if (infoThreadId === threadId) {
      closeInfo();
    }
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
          <SearchInput
            value={historyQuery}
            onChange={setHistoryQuery}
            placeholder="Search recents…"
          />

          <div className="yazg-chat-page__history-header">
            <h2 className="detail-section__title">Recents</h2>
            <div className="yazg-chat-page__history-actions">
              <IconButton
                ariaLabel="New chat"
                size="sm"
                disabled={busy}
                onClick={startNewChat}
              >
                <IconPlus />
              </IconButton>
            </div>
          </div>

          {store.threads.length === 0 ? (
            <p className="text-muted text-sm">No conversations yet.</p>
          ) : historyThreads.length === 0 ? (
            <p className="text-muted text-sm">No matching conversations.</p>
          ) : (
            <ul className="yazg-chat-page__history-list" aria-label="Recent conversations">
              {historyThreads.map((thread) => {
                return (
                  <li key={thread.id}>
                    <div
                      className={`yazg-chat-page__history-item${
                        store.activeThreadId === thread.id
                          ? " yazg-chat-page__history-item--active"
                          : ""
                      }`}
                    >
                      <button
                        type="button"
                        className="yazg-chat-page__history-main"
                        onClick={() => selectThread(thread.id)}
                      >
                        <span className="yazg-chat-page__history-preview">
                          {previewText(thread.title)}
                        </span>
                      </button>
                      <div className="yazg-chat-page__history-menu">
                        <ActionsDropdown
                          label={`Options for ${thread.title}`}
                          disabled={busy}
                          items={[
                            {
                              id: "info",
                              label: "Info",
                              icon: <IconInfo />,
                              onClick: () => openInfo(thread.id),
                            },
                            {
                              id: "rename",
                              label: "Rename",
                              icon: <IconEdit />,
                              onClick: () => openRename(thread.id),
                            },
                            {
                              id: "delete",
                              label: "Delete",
                              icon: <IconTrash />,
                              tone: "danger",
                              onClick: () => deleteThread(thread.id),
                            },
                          ]}
                        />
                      </div>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </Card>

        <Card className="detail-section yazg-chat-page__thread">
          <div className="yazg-chat-page__messages" role="log" aria-live="polite">
            {(activeThread?.messages ?? []).length === 0 && !busy ? (
              <div className="yazg-chat-page__empty">
                <p className="yazg-chat-page__empty-title">
                  Hello, I am Yazg
                  <IconRobot className="yazg-chat-page__empty-icon" />
                </p>
                <p className="yazg-chat-page__empty-hint">How can I help you today?</p>
              </div>
            ) : null}
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
                  <>
                    <hr className="yazg-chat-bubble__divider" />
                    <details className="yazg-chat-thinking">
                      <summary className="yazg-chat-thinking__summary">
                        Thinking
                        <span className="yazg-chat-thinking__count">
                          {msg.events.length}
                        </span>
                      </summary>
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
                    </details>
                  </>
                ) : null}
              </article>
            ))}
            {busy ? (
              <p className="yazg-chat-page__typing text-muted text-sm">Yazg is working…</p>
            ) : null}
            <div ref={bottomRef} />
          </div>

          <div className="yazg-chat-page__composer-wrap">
            <form
              className="yazg-chat-page__composer"
              onSubmit={(event) => {
                event.preventDefault();
                void send(draft, "auto");
              }}
            >
              <div className="yazg-chat-page__input-shell">
                <textarea
                  className="yazg-chat-page__input"
                  rows={1}
                  placeholder="Ask me anything"
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
                <IconButton
                  type="submit"
                  ariaLabel="Send message"
                  variant="primary"
                  size="sm"
                  disabled={!backendConnected || busy || !draft.trim()}
                >
                  <IconSend />
                </IconButton>
              </div>
            </form>
            <p className="yazg-chat-page__composer-hint">
              Yazg is an AI and may make mistakes.
            </p>
          </div>
        </Card>
      </div>

      <Modal
        open={renameThreadId != null}
        title="Rename conversation"
        onClose={closeRename}
        footer={
          <>
            <Button variant="secondary" onClick={closeRename}>
              Cancel
            </Button>
            <Button
              variant="primary"
              disabled={!renameDraft.trim()}
              onClick={submitRename}
            >
              Save
            </Button>
          </>
        }
      >
        <label className="yazg-chat-page__rename-field">
          <span className="yazg-chat-page__label">Title</span>
          <input
            className="yazg-chat-page__rename-input"
            value={renameDraft}
            autoFocus
            maxLength={80}
            onChange={(event) => setRenameDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                submitRename();
              }
            }}
          />
        </label>
      </Modal>

      <Modal
        open={infoThread != null}
        title="Conversation info"
        onClose={closeInfo}
        footer={
          <Button variant="primary" onClick={closeInfo}>
            Close
          </Button>
        }
      >
        {infoThread ? (
          <dl className="yazg-chat-page__info-list">
            <div>
              <dt>Title</dt>
              <dd>{infoThread.title}</dd>
            </div>
            <div>
              <dt>Conversation ID</dt>
              <dd className="mono text-sm">{infoThread.id}</dd>
            </div>
            <div>
              <dt>Created</dt>
              <dd>{new Date(infoThread.createdAt).toLocaleString()}</dd>
            </div>
            <div>
              <dt>Updated</dt>
              <dd>{new Date(infoThread.updatedAt).toLocaleString()}</dd>
            </div>
            <div>
              <dt>Messages</dt>
              <dd>{infoThread.messages.length}</dd>
            </div>
          </dl>
        ) : null}
      </Modal>
    </div>
  );
}
