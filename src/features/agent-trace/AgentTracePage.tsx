import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  yazgChatSessionIds,
  yazgChatThreadTitle,
} from "@/features/yazg/yazgChatSession";
import {
  Button,
  Card,
  EmptyState,
  PageHeader,
  RefreshButton,
  SearchInput,
} from "@/shared/components";
import { toAppError } from "@/shared/errors";
import {
  deleteAgentStmSession,
  listAgentLtm,
  listAgentStmEvents,
  listAgentStmSessions,
  type AgentLtmEntryDto,
  type AgentStmEventDto,
  type AgentStmSessionDto,
} from "@/shared/ipc/agentMemory";
import {
  getRecentLogEvents,
  openLogsFolder,
  tailLogFile,
  type OcsfEventDto,
} from "@/shared/ipc/environment";

import {
  groupStageEvents,
  isTraceTimelineStage,
  prettyJson,
  stageLabel,
  type StageEventLike,
  type StageTraceGroup,
} from "./stageTrace";

/** Tail agents.log for optional LLM wire REQ/RES paired to a conversation. */
const AGENTS_LOG_TAIL_BYTES = 8 * 1024 * 1024;
const YAZG_AGENT_IDS = new Set(["yazg", "Yazg"]);

function formatTime(ts?: string | null): string {
  if (!ts) return "—";
  try {
    return new Date(ts).toLocaleString();
  } catch {
    return ts;
  }
}

function formatRelative(ts?: string | null): string {
  if (!ts) return "—";
  try {
    const then = new Date(ts).getTime();
    const diff = Date.now() - then;
    if (Number.isNaN(diff)) return formatTime(ts);
    const mins = Math.floor(diff / 60_000);
    if (mins < 1) return "just now";
    if (mins < 60) return `${mins}m ago`;
    const hours = Math.floor(mins / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    if (days < 7) return `${days}d ago`;
    return formatTime(ts);
  } catch {
    return ts;
  }
}

function sessionParts(sessionId: string): { title: string; shortId: string } {
  const shortId = sessionId.startsWith("yazg-chat:")
    ? sessionId.slice("yazg-chat:".length)
    : sessionId;
  const title = yazgChatThreadTitle(sessionId)?.trim() || "Untitled chat";
  return { title, shortId };
}

function sessionLabel(sessionId: string): string {
  const { title, shortId } = sessionParts(sessionId);
  return `${title} · ${shortId}`;
}

function parseLogLines(content: string): OcsfEventDto[] {
  return content
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      try {
        return JSON.parse(line) as OcsfEventDto;
      } catch {
        return null;
      }
    })
    .filter((event): event is OcsfEventDto => event !== null);
}

function readConversationId(event: OcsfEventDto): string | null {
  const attrs = event.attributes ?? {};
  for (const key of ["conversationId", "conversation_id", "sessionId", "session_id"]) {
    const value = attrs[key];
    if (typeof value === "string" && value.trim()) return value.trim();
  }
  return null;
}

function isYazgWireEvent(event: OcsfEventDto, conversationId: string): boolean {
  const attrs = event.attributes ?? {};
  const agent =
    (typeof attrs.agent === "string" && attrs.agent) || event.component || "";
  if (!YAZG_AGENT_IDS.has(agent)) return false;
  return readConversationId(event) === conversationId;
}

function toStageEvent(event: OcsfEventDto): StageEventLike {
  const attrs = event.attributes ?? {};
  return {
    agent:
      (typeof attrs.agent === "string" && attrs.agent) ||
      event.component ||
      "yazg",
    kind:
      (typeof attrs.eventKind === "string" && attrs.eventKind) ||
      event.activityName ||
      "info",
    message: event.message,
    timestamp: event.timestamp,
    conversationId: readConversationId(event),
  };
}

function roleBadge(role: string): string {
  switch (role) {
    case "user":
      return "User";
    case "assistant":
      return "Asst";
    case "tool":
      return "Tool";
    case "observation":
      return "Obs";
    case "system":
      return "Sys";
    default:
      return role.slice(0, 4);
  }
}

function familyClass(family: string): string {
  const key = family.toLowerCase();
  if (key.startsWith("capability")) return "capability";
  if (key.startsWith("llm")) return "llm";
  if (key.startsWith("tool")) return "tool";
  if (key.startsWith("completion")) return "completion";
  return "other";
}

function PayloadBody({ text, defaultOpen = false }: { text: string; defaultOpen?: boolean }) {
  const long = text.length > 480 || text.split("\n").length > 12;
  if (!long) {
    return <pre className="agent-trace-payload__body">{text}</pre>;
  }
  return (
    <details className="agent-trace-payload__fold" open={defaultOpen}>
      <summary>
        Payload · {text.length.toLocaleString()} chars
      </summary>
      <pre className="agent-trace-payload__body">{text}</pre>
    </details>
  );
}

function StmEventCard({ event }: { event: AgentStmEventDto }) {
  const body =
    event.contentJson != null
      ? prettyJson({
          content: event.content,
          contentJson: event.contentJson,
        })
      : event.content;

  return (
    <article
      className={`agent-trace-payload agent-trace-payload--stm agent-trace-payload--role-${event.role}`}
    >
      <header className="agent-trace-payload__head">
        <span className="agent-trace-payload__badge">{roleBadge(event.role)}</span>
        <span className="agent-trace-payload__stage">
          {event.memoryKey ? event.memoryKey : event.role}
        </span>
        <span className="agent-trace-payload__meta">
          {formatRelative(event.createdAt)}
        </span>
      </header>
      <PayloadBody text={body} />
    </article>
  );
}

function WirePair({ group }: { group: StageTraceGroup }) {
  if (group.kind !== "pair") return null;
  const family = familyClass(group.family);
  return (
    <article className={`agent-trace-group agent-trace-group--${family}`}>
      <header className="agent-trace-group__head">
        <span className={`agent-trace-group__family agent-trace-group__family--${family}`}>
          {group.family}
        </span>
        <span className="agent-trace-group__stages">
          {stageLabel(group.request.stage, group.request.event.kind)}
          <span aria-hidden="true"> → </span>
          {stageLabel(group.response.stage, group.response.event.kind)}
        </span>
        <span className="agent-trace-group__time">
          {formatRelative(
            group.request.event.timestamp ?? group.response.event.timestamp,
          )}
        </span>
      </header>
      <div className="agent-trace-group__grid">
        <section className="agent-trace-payload agent-trace-payload--req">
          <header className="agent-trace-payload__head">
            <span className="agent-trace-payload__badge">In</span>
            <span className="agent-trace-payload__stage">
              {stageLabel(group.request.stage, group.request.event.kind)}
            </span>
          </header>
          <PayloadBody text={group.request.pretty} defaultOpen />
        </section>
        <section className="agent-trace-payload agent-trace-payload--res">
          <header className="agent-trace-payload__head">
            <span className="agent-trace-payload__badge">Out</span>
            <span className="agent-trace-payload__stage">
              {stageLabel(group.response.stage, group.response.event.kind)}
            </span>
          </header>
          <PayloadBody text={group.response.pretty} defaultOpen />
        </section>
      </div>
    </article>
  );
}

function WireSingle({ group }: { group: StageTraceGroup }) {
  if (group.kind !== "single") return null;
  const { item } = group;
  const badge =
    item.role === "request" ? "In" : item.role === "response" ? "Out" : "Evt";
  const family = familyClass(item.stage ?? item.event.kind);
  return (
    <article className={`agent-trace-group agent-trace-group--${family}`}>
      <header className="agent-trace-group__head">
        <span className={`agent-trace-group__family agent-trace-group__family--${family}`}>
          {item.stage ? stageLabel(item.stage, item.event.kind) : item.event.kind}
        </span>
        <span className="agent-trace-group__time">
          {item.event.timestamp ? formatRelative(item.event.timestamp) : ""}
        </span>
      </header>
      <section
        className={`agent-trace-payload agent-trace-payload--${
          item.role === "response" ? "res" : "req"
        }`}
      >
        <header className="agent-trace-payload__head">
          <span className="agent-trace-payload__badge">{badge}</span>
          <span className="agent-trace-payload__stage">
            {stageLabel(item.stage, item.event.kind)}
          </span>
        </header>
        <PayloadBody text={item.pretty} defaultOpen />
      </section>
    </article>
  );
}

function TimelineGroup({ group }: { group: StageTraceGroup }) {
  if (group.kind === "pair") return <WirePair group={group} />;
  return <WireSingle group={group} />;
}

export function AgentTracePage() {
  const { backendConnected } = useAppStore();
  const [sessions, setSessions] = useState<AgentStmSessionDto[]>([]);
  const [events, setEvents] = useState<AgentStmEventDto[]>([]);
  const [ltm, setLtm] = useState<AgentLtmEntryDto[]>([]);
  const [wireGroups, setWireGroups] = useState<StageTraceGroup[]>([]);
  const [query, setQuery] = useState("");
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);

  async function loadWireForSession(sessionId: string) {
    try {
      const [tail, recent] = await Promise.all([
        tailLogFile("agents.log", AGENTS_LOG_TAIL_BYTES),
        getRecentLogEvents(1500),
      ]);
      const fromFile = parseLogLines(tail.content).filter((e) =>
        isYazgWireEvent(e, sessionId),
      );
      const fromRing = recent.filter((e) => isYazgWireEvent(e, sessionId));
      const map = new Map<string, OcsfEventDto>();
      for (const event of [...fromRing, ...fromFile]) {
        const key = `${event.timestamp}|${event.message.length}|${event.message.slice(0, 80)}`;
        map.set(key, event);
      }
      const merged = Array.from(map.values()).sort((a, b) =>
        a.timestamp.localeCompare(b.timestamp),
      );
      const groups = groupStageEvents(merged.map(toStageEvent)).filter((g) => {
        if (g.kind === "pair") {
          return (
            isTraceTimelineStage(g.request.stage) ||
            isTraceTimelineStage(g.response.stage)
          );
        }
        return isTraceTimelineStage(g.item.stage);
      });
      setWireGroups(groups);
    } catch {
      setWireGroups([]);
    }
  }

  async function refreshSessions() {
    if (!backendConnected) return;
    setLoading(true);
    setError(null);
    try {
      const rows = await listAgentStmSessions({
        prefix: "yazg-chat:",
        limit: 100,
      });
      const liveIds = yazgChatSessionIds();
      const orphans = rows.filter((row) => !liveIds.has(row.sessionId));
      if (orphans.length > 0) {
        await Promise.all(
          orphans.map((row) =>
            deleteAgentStmSession(row.sessionId).catch(() => null),
          ),
        );
      }
      const live = rows.filter((row) => liveIds.has(row.sessionId));
      setSessions(live);
      if (live.length === 0) {
        setSelectedSessionId(null);
        setEvents([]);
        setLtm([]);
        setWireGroups([]);
      } else if (
        !selectedSessionId ||
        !live.some((row) => row.sessionId === selectedSessionId)
      ) {
        setSelectedSessionId(live[0]!.sessionId);
      }
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setLoading(false);
    }
  }

  async function refreshSelected(sessionId: string) {
    if (!backendConnected) return;
    setLoading(true);
    setError(null);
    try {
      const [stmEvents, ltmRows] = await Promise.all([
        listAgentStmEvents({ sessionId, limit: 500 }),
        listAgentLtm({
          agentId: "yazg",
          scopeType: "global",
          scopeId: "",
          limit: 32,
        }),
      ]);
      setEvents(stmEvents);
      setLtm(
        ltmRows.filter(
          (row) =>
            row.memoryKey.startsWith("conversation.") ||
            (typeof row.content === "string" &&
              row.content.includes(sessionId)),
        ),
      );
      await loadWireForSession(sessionId);
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setLoading(false);
    }
  }

  async function removeSession(sessionId: string) {
    if (!backendConnected) return;
    const confirmed = window.confirm(
      `Delete STM for ${sessionLabel(sessionId)}?\nThis removes Agent Trace data for that conversation.`,
    );
    if (!confirmed) return;
    setLoading(true);
    setError(null);
    try {
      await deleteAgentStmSession(sessionId);
      if (selectedSessionId === sessionId) {
        setSelectedSessionId(null);
        setEvents([]);
        setLtm([]);
        setWireGroups([]);
      }
      await refreshSessions();
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refreshSessions();
  }, [backendConnected]);

  useEffect(() => {
    if (!selectedSessionId || !backendConnected) return;
    void refreshSelected(selectedSessionId);
  }, [selectedSessionId, backendConnected]);

  useEffect(() => {
    if (!autoRefresh || !backendConnected) return;
    const timer = window.setInterval(() => {
      void refreshSessions();
      if (selectedSessionId) void refreshSelected(selectedSessionId);
    }, 5000);
    return () => window.clearInterval(timer);
  }, [autoRefresh, backendConnected, selectedSessionId]);

  const filteredSessions = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return sessions;
    return sessions.filter((session) => {
      const { title, shortId } = sessionParts(session.sessionId);
      return (
        session.sessionId.toLowerCase().includes(q) ||
        title.toLowerCase().includes(q) ||
        shortId.toLowerCase().includes(q)
      );
    });
  }, [sessions, query]);

  const filteredEvents = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return events;
    return events.filter(
      (event) =>
        event.content.toLowerCase().includes(q) ||
        event.role.toLowerCase().includes(q) ||
        (event.memoryKey ?? "").toLowerCase().includes(q),
    );
  }, [events, query]);

  const selected =
    filteredSessions.find((s) => s.sessionId === selectedSessionId) ??
    filteredSessions[0] ??
    null;

  const selectedParts = selected ? sessionParts(selected.sessionId) : null;
  const pairCount = wireGroups.filter((g) => g.kind === "pair").length;
  const singleCount = wireGroups.filter((g) => g.kind === "single").length;

  return (
    <div className="agent-trace-page">
      <PageHeader
        title="Agent Trace"
        description="Inspect Yazg turns still live in Assistant — capability routing, LLM wire, tools, and STM."
        actions={
          <>
            <label
              className={`agent-trace-page__live${
                autoRefresh ? " agent-trace-page__live--on" : ""
              }`}
            >
              <input
                type="checkbox"
                checked={autoRefresh}
                onChange={(event) => setAutoRefresh(event.target.checked)}
              />
              <span className="agent-trace-page__live-dot" aria-hidden="true" />
              Live
            </label>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => void openLogsFolder()}
              disabled={!backendConnected}
            >
              Logs folder
            </Button>
            <RefreshButton
              onClick={() => {
                void refreshSessions();
                if (selectedSessionId) void refreshSelected(selectedSessionId);
              }}
              loading={loading}
              error={error}
              showSuccessToast={false}
            />
          </>
        }
      />

      {!backendConnected ? (
        <Card className="agent-trace-page__banner">
          <EmptyState
            title="Backend offline"
            description="Connect the desktop backend to read STM from SQLite and pair stages from agents.log."
          />
        </Card>
      ) : null}

      {error ? (
        <Card className="agent-trace-page__banner agent-trace-page__banner--error">
          <p className="agent-trace-page__error">{error}</p>
        </Card>
      ) : null}

      <div className="agent-trace-page__layout">
        <aside className="agent-trace-page__rail">
          <div className="agent-trace-page__rail-head">
            <div>
              <p className="agent-trace-page__eyebrow">Sessions</p>
              <h3 className="agent-trace-page__rail-title">Conversations</h3>
            </div>
            <span className="agent-trace-page__metric">
              {filteredSessions.length}
            </span>
          </div>

          <SearchInput
            value={query}
            onChange={setQuery}
            placeholder="Filter by title or id…"
          />

          {filteredSessions.length === 0 ? (
            <EmptyState
              title="No live traces"
              description="Chat in Yazg to append STM. Deleted Assistant threads are pruned here."
              action={
                <Link className="btn btn--secondary btn--sm" to="/yazg">
                  Open Yazg
                </Link>
              }
            />
          ) : (
            <ul className="agent-trace-page__list">
              {filteredSessions.map((session) => {
                const active = session.sessionId === selected?.sessionId;
                const { title, shortId } = sessionParts(session.sessionId);
                return (
                  <li key={session.sessionId}>
                    <div
                      className={`agent-trace-page__session${
                        active ? " agent-trace-page__session--active" : ""
                      }`}
                    >
                      <button
                        type="button"
                        className="agent-trace-page__session-main"
                        onClick={() => setSelectedSessionId(session.sessionId)}
                      >
                        <span className="agent-trace-page__session-title">
                          {title}
                        </span>
                        <span
                          className="agent-trace-page__session-id"
                          title={session.sessionId}
                        >
                          {shortId}
                        </span>
                        <span className="agent-trace-page__session-meta">
                          <span className="agent-trace-page__session-count">
                            {session.eventCount} evt
                          </span>
                          <span>{formatRelative(session.lastAt)}</span>
                        </span>
                      </button>
                      <button
                        type="button"
                        className="agent-trace-page__session-delete"
                        title="Delete STM for this conversation"
                        onClick={() => void removeSession(session.sessionId)}
                      >
                        Remove
                      </button>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </aside>

        <section className="agent-trace-page__detail">
          {!selected || !selectedParts ? (
            <EmptyState
              title="Select a conversation"
              description="Pick a live Yazg session to inspect classify → LLM → tool stages and STM."
            />
          ) : (
            <>
              <header className="agent-trace-page__detail-head">
                <div className="agent-trace-page__detail-copy">
                  <p className="agent-trace-page__eyebrow">Trace</p>
                  <h3 className="agent-trace-page__detail-title">
                    {selectedParts.title}
                  </h3>
                  <p
                    className="agent-trace-page__detail-id"
                    title={selected.sessionId}
                  >
                    {selected.sessionId}
                  </p>
                </div>
                <dl className="agent-trace-page__stats">
                  <div>
                    <dt>Pairs</dt>
                    <dd>{pairCount}</dd>
                  </div>
                  <div>
                    <dt>Singles</dt>
                    <dd>{singleCount}</dd>
                  </div>
                  <div>
                    <dt>STM</dt>
                    <dd>{filteredEvents.length}</dd>
                  </div>
                  <div>
                    <dt>Updated</dt>
                    <dd>{formatRelative(selected.lastAt)}</dd>
                  </div>
                </dl>
              </header>

              <div className="agent-trace-page__detail-stack">
                <section className="agent-trace-page__panel">
                  <header className="agent-trace-page__panel-head">
                    <div>
                      <p className="agent-trace-page__eyebrow">agents.log</p>
                      <h4 className="agent-trace-page__panel-title">
                        Stage timeline
                      </h4>
                    </div>
                    <span className="agent-trace-page__metric">
                      {wireGroups.length}
                    </span>
                  </header>
                  <p className="agent-trace-page__panel-hint">
                    Capability classify, then the OpenAI-style LLM wire. Rig
                    completion hooks are omitted — they duplicate the same turn.
                  </p>
                  {wireGroups.length === 0 ? (
                    <EmptyState
                      title="No stage pairs yet"
                      description="Send a Yazg message, then refresh. Fully redacted older log lines are skipped."
                    />
                  ) : (
                    <div className="agent-trace-page__timeline">
                      {wireGroups.map((group) => (
                        <TimelineGroup key={group.id} group={group} />
                      ))}
                    </div>
                  )}
                </section>

                <section className="agent-trace-page__panel">
                  <header className="agent-trace-page__panel-head">
                    <div>
                      <p className="agent-trace-page__eyebrow">SQLite</p>
                      <h4 className="agent-trace-page__panel-title">
                        Short-term memory
                      </h4>
                    </div>
                    <span className="agent-trace-page__metric">
                      {filteredEvents.length}
                    </span>
                  </header>
                  {filteredEvents.length === 0 ? (
                    <EmptyState
                      title="Empty STM"
                      description="No short-term memory rows for this session."
                    />
                  ) : (
                    <div className="agent-trace-page__stm-list">
                      {filteredEvents.map((event) => (
                        <StmEventCard key={event.id} event={event} />
                      ))}
                    </div>
                  )}
                </section>

                {ltm.length > 0 ? (
                  <details className="agent-trace-raw">
                    <summary>Long-term memory insights</summary>
                    <pre className="agent-trace-payload__body">
                      {prettyJson(ltm)}
                    </pre>
                  </details>
                ) : null}
              </div>
            </>
          )}
        </section>
      </div>
    </div>
  );
}
