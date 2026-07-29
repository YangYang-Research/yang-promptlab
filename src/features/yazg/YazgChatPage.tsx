import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";

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
import { useAiInferenceRoute } from "@/shared/hooks/useAiInferenceRoute";
import { healthCheck } from "@/shared/ipc";
import { isYazgAgentLive } from "@/shared/runtime/yazgAgentLive";
import { useToast } from "@/shared/notifications";
import { useAppStore } from "@/app/store/AppStore";

import { YazgMarkdown } from "./YazgMarkdown";
import {
  deleteYazgChatThread,
  getYazgChatSessionSnapshot,
  renameYazgChatThread,
  selectYazgChatThread,
  sendYazgChatMessage,
  setYazgChatHostHooks,
  startNewYazgChat,
  subscribeYazgChatSession,
} from "./yazgChatSession";

function previewText(text: string, max = 72): string {
  const trimmed = text.trim().replace(/\s+/g, " ");
  if (trimmed.length <= max) return trimmed;
  return `${trimmed.slice(0, max - 1)}…`;
}

function formatRawOutput(raw: unknown): string {
  try {
    return JSON.stringify(raw, null, 2);
  } catch {
    return String(raw);
  }
}

export function YazgChatPage() {
  const { actions } = useAppStore();
  const [backendConnected, setBackendConnected] = useState(false);
  const { configuration, loading: configLoading } = useAiInferenceRoute({
    enabled: backendConnected,
  });
  const { notify } = useToast();
  const live = isYazgAgentLive(configuration);

  const session = useSyncExternalStore(
    subscribeYazgChatSession,
    getYazgChatSessionSnapshot,
    getYazgChatSessionSnapshot,
  );
  const { store, busy, pendingThreadId } = session;

  const [draft, setDraft] = useState("");
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

  const activePending =
    busy && pendingThreadId != null && pendingThreadId === activeThread?.id;

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

  // Keep hooks after unmount so in-flight replies can still toast / refresh.
  useEffect(() => {
    setYazgChatHostHooks({
      notify,
      refresh: () => {
        void actions.refresh();
      },
    });
  }, [actions, notify]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [activeThread?.id, activeThread?.messages.length, activePending]);

  const startNewChat = () => {
    startNewYazgChat();
    setDraft("");
  };

  const selectThread = (threadId: string) => {
    selectYazgChatThread(threadId);
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
    renameYazgChatThread(renameThreadId, renameDraft);
    closeRename();
  };

  const deleteThread = (threadId: string) => {
    deleteYazgChatThread(threadId);
    if (renameThreadId === threadId) {
      closeRename();
    }
    if (infoThreadId === threadId) {
      closeInfo();
    }
  };

  const send = async (message: string) => {
    const trimmed = message.trim();
    if (!trimmed || busy) return;
    setDraft("");
    await sendYazgChatMessage({
      message: trimmed,
      intent: "auto",
      backendConnected,
    });
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
                const threadPending = busy && pendingThreadId === thread.id;
                return (
                  <li key={thread.id}>
                    <div
                      className={`yazg-chat-page__history-item${
                        store.activeThreadId === thread.id
                          ? " yazg-chat-page__history-item--active"
                          : ""
                      }${threadPending ? " yazg-chat-page__history-item--pending" : ""}`}
                    >
                      <button
                        type="button"
                        className="yazg-chat-page__history-main"
                        onClick={() => selectThread(thread.id)}
                      >
                        <span className="yazg-chat-page__history-preview">
                          {previewText(thread.title)}
                          {threadPending ? " · …" : ""}
                        </span>
                      </button>
                      <div className="yazg-chat-page__history-menu">
                        <ActionsDropdown
                          label={`Options for ${thread.title}`}
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
                              disabled: threadPending,
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
            {(activeThread?.messages ?? []).length === 0 && !activePending ? (
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
                <YazgMarkdown className="yazg-chat-bubble__text yazg-chat-md" text={msg.text} />
                {msg.rawOutput != null ? (
                  <>
                    <hr className="yazg-chat-bubble__divider" />
                    <details className="yazg-chat-thinking">
                      <summary className="yazg-chat-thinking__summary">
                        Agent trace
                      </summary>
                      <pre className="yazg-chat-events__msg">
                        {formatRawOutput(msg.rawOutput)}
                      </pre>
                    </details>
                  </>
                ) : null}
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
                            <pre className="yazg-chat-events__msg">{event.message}</pre>
                          </li>
                        ))}
                      </ul>
                    </details>
                  </>
                ) : null}
              </article>
            ))}
            {activePending ? (
              <p className="yazg-chat-page__typing text-muted text-sm">Yazg is working…</p>
            ) : null}
            <div ref={bottomRef} />
          </div>

          <div className="yazg-chat-page__composer-wrap">
            <form
              className="yazg-chat-page__composer"
              onSubmit={(event) => {
                event.preventDefault();
                void send(draft);
              }}
            >
              <div className="yazg-chat-page__input-shell">
                <textarea
                  className="yazg-chat-page__input"
                  rows={3}
                  placeholder="Ask me anything"
                  value={draft}
                  disabled={!backendConnected || busy}
                  onChange={(event) => setDraft(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" && !event.shiftKey) {
                      event.preventDefault();
                      void send(draft);
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
