import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { Link } from "react-router-dom";

import {
  ActionsDropdown,
  Button,
  Card,
  IconButton,
  IconEdit,
  IconExternalLink,
  IconInfo,
  IconPlus,
  IconRobot,
  IconSend,
  IconStop,
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
  resolveYazgHiltAction,
  selectYazgChatThread,
  sendYazgChatMessage,
  setYazgChatHostHooks,
  startNewYazgChat,
  stopYazgChat,
  subscribeYazgChatSession,
} from "./yazgChatSession";
import type { YazgHiltPendingActionDto } from "@/shared/ipc/yazg";

function previewText(text: string, max = 72): string {
  const trimmed = text.trim().replace(/\s+/g, " ");
  if (trimmed.length <= max) return trimmed;
  return `${trimmed.slice(0, max - 1)}…`;
}

function formatHiltRemaining(ms: number): string {
  const totalSec = Math.max(0, Math.ceil(ms / 1000));
  const m = Math.floor(totalSec / 60);
  const s = totalSec % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

function HiltCountdownBar({
  pending,
  onExpire,
}: {
  pending: YazgHiltPendingActionDto;
  onExpire: () => void;
}) {
  const [now, setNow] = useState(() => Date.now());
  const expiredRef = useRef(false);

  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 250);
    return () => window.clearInterval(id);
  }, []);

  const total = Math.max(1, pending.expiresAtMs - pending.createdAtMs);
  const left = Math.max(0, pending.expiresAtMs - now);
  const pct = Math.min(100, Math.max(0, (left / total) * 100));

  useEffect(() => {
    if (left > 0 || expiredRef.current) return;
    expiredRef.current = true;
    onExpire();
  }, [left, onExpire]);

  return (
    <div className="yazg-hilt-card__ttl" aria-label={`Expires in ${formatHiltRemaining(left)}`}>
      <div className="yazg-hilt-card__ttl-meta">
        <span>Expires in {formatHiltRemaining(left)}</span>
      </div>
      <div className="yazg-hilt-card__ttl-track" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(pct)}>
        <div className="yazg-hilt-card__ttl-fill" style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
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
  const { store, busy, stopping, pendingThreadId } = session;

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
                  <span className="yazg-chat-bubble__meta-label">
                    {msg.role === "yazg" ? "Yazg" : "You"}
                    {(msg.action ?? msg.intent) ? (
                      <span className="yazg-chat-bubble__intent">
                        {msg.action ?? msg.intent}
                      </span>
                    ) : null}
                  </span>
                  {msg.role === "yazg" && msg.traceId ? (
                    <Link
                      className="yazg-chat-bubble__trace-link"
                      to={`/agent-trace/${encodeURIComponent(msg.traceId)}`}
                      title={`Open trace ${msg.traceId}`}
                      aria-label="Open Agent Trace for this reply"
                    >
                      <IconExternalLink />
                    </Link>
                  ) : null}
                </header>
                <YazgMarkdown className="yazg-chat-bubble__text yazg-chat-md" text={msg.text} />
                {msg.role === "yazg" && msg.pendingAction && !msg.hiltDecision ? (
                  <div className="yazg-hilt-card" role="group" aria-label="Confirm action">
                    <p className="yazg-hilt-card__summary">{msg.pendingAction.summary}</p>
                    <p className="yazg-hilt-card__meta text-muted text-sm">
                      {msg.pendingAction.kind} · {msg.pendingAction.tool}
                    </p>
                    <div className="yazg-hilt-card__actions">
                      <Button
                        variant="secondary"
                        size="sm"
                        disabled={!backendConnected || busy}
                        onClick={() => {
                          if (!activeThread) return;
                          void resolveYazgHiltAction({
                            threadId: activeThread.id,
                            messageId: msg.id,
                            actionId: msg.pendingAction!.id,
                            decision: "deny",
                            backendConnected,
                          });
                        }}
                      >
                        Deny
                      </Button>
                      <Button
                        variant="primary"
                        size="sm"
                        disabled={!backendConnected || busy}
                        onClick={() => {
                          if (!activeThread) return;
                          void resolveYazgHiltAction({
                            threadId: activeThread.id,
                            messageId: msg.id,
                            actionId: msg.pendingAction!.id,
                            decision: "approve",
                            backendConnected,
                          });
                        }}
                      >
                        Approve
                      </Button>
                    </div>
                    <HiltCountdownBar
                      pending={msg.pendingAction}
                      onExpire={() => {
                        if (!activeThread) return;
                        void resolveYazgHiltAction({
                          threadId: activeThread.id,
                          messageId: msg.id,
                          actionId: msg.pendingAction!.id,
                          decision: "expire",
                          backendConnected,
                        });
                      }}
                    />
                  </div>
                ) : null}
                {msg.role === "yazg" && msg.hiltDecision ? (
                  <p className="yazg-hilt-card__resolved text-muted text-sm">
                    {msg.hiltDecision === "approve"
                      ? "Approved"
                      : msg.hiltDecision === "expire"
                        ? "Expired"
                        : "Denied"}
                  </p>
                ) : null}
              </article>
            ))}
            {activePending ? (
              <p className="yazg-chat-page__typing text-muted text-sm">
                {stopping ? "Stopping…" : "Yazg is working…"}
              </p>
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
                {busy ? (
                  <IconButton
                    type="button"
                    ariaLabel={stopping ? "Stopping" : "Stop"}
                    variant="danger"
                    size="sm"
                    disabled={!backendConnected || stopping}
                    onClick={() => {
                      void stopYazgChat();
                    }}
                  >
                    <IconStop />
                  </IconButton>
                ) : (
                  <IconButton
                    type="submit"
                    ariaLabel="Send message"
                    variant="primary"
                    size="sm"
                    disabled={!backendConnected || !draft.trim()}
                  >
                    <IconSend />
                  </IconButton>
                )}
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
