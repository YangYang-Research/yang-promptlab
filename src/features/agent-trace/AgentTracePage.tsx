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

function sessionLabel(sessionId: string): string {
  const title = yazgChatThreadTitle(sessionId);
  const shortId = sessionId.startsWith("yazg-chat:")
    ? sessionId.slice("yazg-chat:".length)
    : sessionId;
  return title ? `${title} · ${shortId}` : shortId;
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
      return "USER";
    case "assistant":
      return "ASST";
    case "tool":
      return "TOOL";
    case "observation":
      return "OBS";
    case "system":
      return "SYS";
    default:
      return role.slice(0, 4).toUpperCase();
  }
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
    <section
      className={`agent-trace-payload agent-trace-payload--stm agent-trace-payload--role-${event.role}`}
    >
      <header className="agent-trace-payload__head">
        <span className="agent-trace-payload__badge">{roleBadge(event.role)}</span>
        <span className="agent-trace-payload__stage">
          {event.memoryKey ? `${event.role} · ${event.memoryKey}` : event.role}
        </span>
        <span className="agent-trace-payload__meta">
          {event.agentId}
          {event.createdAt ? ` · ${event.createdAt}` : ""}
        </span>
      </header>
      <pre className="agent-trace-payload__body">{body}</pre>
    </section>
  );
}

function WirePair({ group }: { group: StageTraceGroup }) {
  if (group.kind !== "pair") return null;
  return (
    <article className="agent-trace-group">
      <header className="agent-trace-group__head">
        <strong>{group.family}</strong>
        <span>
          {stageLabel(group.request.stage, group.request.event.kind)} →{" "}
          {stageLabel(group.response.stage, group.response.event.kind)}
        </span>
      </header>
      <div className="agent-trace-group__grid">
        <section className="agent-trace-payload agent-trace-payload--req">
          <header className="agent-trace-payload__head">
            <span className="agent-trace-payload__badge">REQ</span>
            <span className="agent-trace-payload__stage">
              {stageLabel(group.request.stage, group.request.event.kind)}
            </span>
            {group.request.event.timestamp ? (
              <span className="agent-trace-payload__meta">
                {formatTime(group.request.event.timestamp)}
              </span>
            ) : null}
          </header>
          <pre className="agent-trace-payload__body">{group.request.pretty}</pre>
        </section>
        <section className="agent-trace-payload agent-trace-payload--res">
          <header className="agent-trace-payload__head">
            <span className="agent-trace-payload__badge">RES</span>
            <span className="agent-trace-payload__stage">
              {stageLabel(group.response.stage, group.response.event.kind)}
            </span>
            {group.response.event.timestamp ? (
              <span className="agent-trace-payload__meta">
                {formatTime(group.response.event.timestamp)}
              </span>
            ) : null}
          </header>
          <pre className="agent-trace-payload__body">{group.response.pretty}</pre>
        </section>
      </div>
    </article>
  );
}

function WireSingle({ group }: { group: StageTraceGroup }) {
  if (group.kind !== "single") return null;
  const { item } = group;
  const badge =
    item.role === "request" ? "REQ" : item.role === "response" ? "RES" : "EVT";
  return (
    <article className="agent-trace-group">
      <header className="agent-trace-group__head">
        <strong>{item.stage ? stageLabel(item.stage, item.event.kind) : item.event.kind}</strong>
        <span>{item.event.timestamp ? formatTime(item.event.timestamp) : ""}</span>
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
        <pre className="agent-trace-payload__body">{item.pretty}</pre>
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
      const label = sessionLabel(session.sessionId).toLowerCase();
      return (
        session.sessionId.toLowerCase().includes(q) || label.includes(q)
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

  const pairCount = wireGroups.filter((g) => g.kind === "pair").length;
  const singleCount = wireGroups.filter((g) => g.kind === "single").length;

  return (
    <div className="agent-trace-page">
      <PageHeader
        title="Agent Trace"
        description="Yazg conversations still in Assistant. Timeline = capability classify + LLM wire + tool/completion stages from agents.log; STM = SQLite short-term memory."
        actions={
          <>
            <label className="agent-trace-page__auto">
              <input
                type="checkbox"
                checked={autoRefresh}
                onChange={(event) => setAutoRefresh(event.target.checked)}
              />
              Auto-refresh
            </label>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => void openLogsFolder()}
              disabled={!backendConnected}
            >
              Open logs folder
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
        <Card className="detail-section">
          <p className="text-muted">Connect the desktop backend to load STM from SQLite.</p>
        </Card>
      ) : null}

      {error ? (
        <Card className="detail-section">
          <p className="text-danger">{error}</p>
        </Card>
      ) : null}

      <div className="agent-trace-page__toolbar">
        <SearchInput
          value={query}
          onChange={setQuery}
          placeholder="Search title, conversation ID, or STM content…"
        />
        <span className="agent-trace-page__count text-muted text-sm">
          STM · {filteredSessions.length} conversations
          {selected ? ` · ${filteredEvents.length} events` : ""}
          {selected && wireGroups.length > 0
            ? ` · ${pairCount} pairs / ${singleCount} singles`
            : ""}
        </span>
      </div>

      <div className="agent-trace-page__layout">
        <Card className="detail-section agent-trace-page__list-card">
          <h3 className="agent-trace-page__list-title">Conversations (STM)</h3>
          {filteredSessions.length === 0 ? (
            <p className="text-muted text-sm">
              No Yazg STM sessions for live Assistant chats. Chat in Yazg to
              append session events. <Link to="/yazg">Open Yazg</Link>
            </p>
          ) : (
            <ul className="agent-trace-page__list">
              {filteredSessions.map((session) => {
                const active = session.sessionId === selected?.sessionId;
                return (
                  <li key={session.sessionId} className="agent-trace-page__list-row">
                    <button
                      type="button"
                      className={`agent-trace-page__list-item agent-trace-page__list-item--conversation${
                        active ? " agent-trace-page__list-item--active" : ""
                      }`}
                      onClick={() => setSelectedSessionId(session.sessionId)}
                    >
                      <span className="agent-trace-page__list-agent">
                        {session.eventCount} evt
                      </span>
                      <span
                        className="agent-trace-page__list-label"
                        title={session.sessionId}
                      >
                        {sessionLabel(session.sessionId)}
                      </span>
                      <span className="agent-trace-page__list-time">
                        {formatTime(session.lastAt)}
                      </span>
                    </button>
                    <button
                      type="button"
                      className="agent-trace-page__list-delete"
                      title="Delete STM for this conversation"
                      onClick={(event) => {
                        event.stopPropagation();
                        void removeSession(session.sessionId);
                      }}
                    >
                      Delete
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </Card>

        <Card className="detail-section agent-trace-page__detail-card">
          <h3 className="agent-trace-page__list-title">
            {selected
              ? `Trace · ${sessionLabel(selected.sessionId)}`
              : "Trace detail"}
          </h3>
          {selected ? (
            <div className="agent-trace-page__detail-stack">
              <div className="agent-trace-page__wire">
                <h4 className="agent-trace-page__list-title">
                  Stage timeline ({wireGroups.length})
                </h4>
                <p className="text-muted text-sm agent-trace-page__hint">
                  Capability classify → LLM wire → tool/completion (from agents.log).
                </p>
                {wireGroups.length === 0 ? (
                  <p className="text-muted text-sm">
                    No stage timeline pairs yet for this conversation. Send a
                    new Yazg message, then refresh. (Older fully `[REDACTED]` log
                    lines are skipped.)
                  </p>
                ) : (
                  wireGroups.map((group) => (
                    <TimelineGroup key={group.id} group={group} />
                  ))
                )}
              </div>

              <h4 className="agent-trace-page__list-title">STM events</h4>
              {filteredEvents.length === 0 ? (
                <p className="text-muted text-sm">No events in this session.</p>
              ) : (
                filteredEvents.map((event) => (
                  <StmEventCard key={event.id} event={event} />
                ))
              )}

              {ltm.length > 0 ? (
                <details className="agent-trace-raw">
                  <summary>LTM insights (extracted)</summary>
                  <pre className="agent-trace-payload__body">
                    {prettyJson(ltm)}
                  </pre>
                </details>
              ) : null}
            </div>
          ) : (
            <p className="text-muted text-sm">Select a conversation.</p>
          )}
        </Card>
      </div>
    </div>
  );
}
