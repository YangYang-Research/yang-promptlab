import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  Button,
  Card,
  PageHeader,
  RefreshButton,
  SearchInput,
  Select,
} from "@/shared/components";
import { toAppError } from "@/shared/errors";
import {
  getRecentLogEvents,
  openLogsFolder,
  tailLogFile,
  type OcsfEventDto,
} from "@/shared/ipc/environment";

import {
  groupStageEvents,
  prettyJson,
  stageLabel,
  type ParsedStageEvent,
  type StageEventLike,
  type StageTraceGroup,
} from "./stageTrace";

/** Tail enough of agents.log to keep full LLM request/response bodies. */
const AGENTS_LOG_TAIL_BYTES = 16 * 1024 * 1024;

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

function isAgentEvent(event: OcsfEventDto): boolean {
  if (event.category === "agent") return true;
  if (event.module === "promptlab-agent") return true;
  const attrs = event.attributes ?? {};
  return typeof attrs.agent === "string" || typeof attrs.eventKind === "string";
}

function toStageEvent(event: OcsfEventDto): StageEventLike {
  const attrs = event.attributes ?? {};
  const agent =
    (typeof attrs.agent === "string" && attrs.agent) ||
    event.component ||
    "unknown";
  const kind =
    (typeof attrs.eventKind === "string" && attrs.eventKind) ||
    event.activityName ||
    "info";
  return {
    agent,
    kind,
    message: event.message,
    timestamp: event.timestamp,
  };
}

function StagePayload({
  item,
  badge,
}: {
  item: ParsedStageEvent;
  badge?: "REQ" | "RES";
}) {
  return (
    <section
      className={`agent-trace-payload${
        badge === "REQ"
          ? " agent-trace-payload--req"
          : badge === "RES"
            ? " agent-trace-payload--res"
            : ""
      }`}
    >
      <header className="agent-trace-payload__head">
        {badge ? <span className="agent-trace-payload__badge">{badge}</span> : null}
        <span className="agent-trace-payload__stage">
          {stageLabel(item.stage, item.event.kind)}
        </span>
        <span className="agent-trace-payload__meta">
          {item.event.agent} · {item.event.kind}
          {item.event.timestamp ? ` · ${item.event.timestamp}` : ""}
        </span>
      </header>
      <pre className="agent-trace-payload__body">{item.pretty}</pre>
    </section>
  );
}

function TraceGroupView({ group }: { group: StageTraceGroup }) {
  if (group.kind === "pair") {
    return (
      <article className="agent-trace-group">
        <header className="agent-trace-group__head">
          <strong>{group.family}</strong>
          <span>request → response</span>
        </header>
        <div className="agent-trace-group__grid">
          <StagePayload item={group.request} badge="REQ" />
          <StagePayload item={group.response} badge="RES" />
        </div>
      </article>
    );
  }

  const badge =
    group.item.role === "request"
      ? "REQ"
      : group.item.role === "response"
        ? "RES"
        : undefined;
  return (
    <article className="agent-trace-group">
      <StagePayload item={group.item} badge={badge} />
    </article>
  );
}

export function AgentTracePage() {
  const { backendConnected } = useAppStore();
  const [events, setEvents] = useState<OcsfEventDto[]>([]);
  const [query, setQuery] = useState("");
  const [agentFilter, setAgentFilter] = useState("all");
  const [kindFilter, setKindFilter] = useState("all");
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedGroupId, setSelectedGroupId] = useState<string | null>(null);

  async function refresh() {
    if (!backendConnected) return;
    setLoading(true);
    setError(null);
    try {
      const [tail, recent] = await Promise.all([
        tailLogFile("agents.log", AGENTS_LOG_TAIL_BYTES),
        getRecentLogEvents(2000),
      ]);
      const fromFile = parseLogLines(tail.content).filter(isAgentEvent);
      const fromRing = recent.filter(isAgentEvent);
      const map = new Map<string, OcsfEventDto>();
      for (const event of [...fromRing, ...fromFile]) {
        const key = `${event.timestamp}|${event.component}|${event.activityName}|${event.message.length}|${event.message.slice(0, 120)}`;
        map.set(key, event);
      }
      const merged = Array.from(map.values()).sort((a, b) =>
        a.timestamp.localeCompare(b.timestamp),
      );
      setEvents(merged);
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, [backendConnected]);

  useEffect(() => {
    if (!autoRefresh || !backendConnected) return;
    const timer = window.setInterval(() => void refresh(), 4000);
    return () => window.clearInterval(timer);
  }, [autoRefresh, backendConnected]);

  const stageEvents = useMemo(() => events.map(toStageEvent), [events]);

  const agents = useMemo(() => {
    const set = new Set(stageEvents.map((e) => e.agent).filter(Boolean));
    return Array.from(set).sort();
  }, [stageEvents]);

  const kinds = useMemo(() => {
    const set = new Set(stageEvents.map((e) => e.kind).filter(Boolean));
    return Array.from(set).sort();
  }, [stageEvents]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return stageEvents.filter((event) => {
      if (agentFilter !== "all" && event.agent !== agentFilter) return false;
      if (kindFilter !== "all" && event.kind !== kindFilter) return false;
      if (!q) return true;
      return (
        event.message.toLowerCase().includes(q) ||
        event.agent.toLowerCase().includes(q) ||
        event.kind.toLowerCase().includes(q) ||
        (event.timestamp ?? "").toLowerCase().includes(q)
      );
    });
  }, [stageEvents, query, agentFilter, kindFilter]);

  const groups = useMemo(() => groupStageEvents(filtered), [filtered]);

  useEffect(() => {
    if (groups.length === 0) {
      setSelectedGroupId(null);
      return;
    }
    if (!selectedGroupId || !groups.some((g) => g.id === selectedGroupId)) {
      setSelectedGroupId(groups[groups.length - 1]!.id);
    }
  }, [groups, selectedGroupId]);

  const selectedGroup =
    groups.find((group) => group.id === selectedGroupId) ?? groups[groups.length - 1] ?? null;

  return (
    <div className="agent-trace-page">
      <PageHeader
        title="Agent Trace"
        description="Full LLM request/response and tool stages for every agent. Bodies are not truncated."
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
              onClick={() => void refresh()}
              loading={loading}
              error={error}
              showSuccessToast={false}
            />
          </>
        }
      />

      {!backendConnected ? (
        <Card className="detail-section">
          <p className="text-muted">Connect the desktop backend to load `agents.log`.</p>
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
          placeholder="Search stage, agent, body…"
        />
        <Select value={agentFilter} onChange={(e) => setAgentFilter(e.target.value)}>
          <option value="all">All agents</option>
          {agents.map((agent) => (
            <option key={agent} value={agent}>
              {agent}
            </option>
          ))}
        </Select>
        <Select value={kindFilter} onChange={(e) => setKindFilter(e.target.value)}>
          <option value="all">All kinds</option>
          {kinds.map((kind) => (
            <option key={kind} value={kind}>
              {kind}
            </option>
          ))}
        </Select>
        <span className="agent-trace-page__count text-muted text-sm">
          {groups.length} groups · {filtered.length} events
          {filtered.length !== stageEvents.length
            ? ` (of ${stageEvents.length})`
            : ""}
        </span>
      </div>

      <div className="agent-trace-page__layout">
        <Card className="detail-section agent-trace-page__list-card">
          <h3 className="agent-trace-page__list-title">Timeline</h3>
          {groups.length === 0 ? (
            <p className="text-muted text-sm">
              No agent events yet. Run Yazg or another agent, then refresh. Source:{" "}
              <code>~/.promptlab/logs/agents.log</code>. Chat UI no longer embeds traces — use
              this page. <Link to="/yazg">Open Yazg</Link>
            </p>
          ) : (
            <ul className="agent-trace-page__list">
              {groups.map((group) => {
                const label =
                  group.kind === "pair"
                    ? `${group.request.stage} → ${group.response.stage}`
                    : stageLabel(group.item.stage, group.item.event.kind);
                const agent =
                  group.kind === "pair"
                    ? group.request.event.agent
                    : group.item.event.agent;
                const ts =
                  group.kind === "pair"
                    ? group.request.event.timestamp
                    : group.item.event.timestamp;
                const active = group.id === selectedGroup?.id;
                return (
                  <li key={group.id}>
                    <button
                      type="button"
                      className={`agent-trace-page__list-item${
                        active ? " agent-trace-page__list-item--active" : ""
                      }`}
                      onClick={() => setSelectedGroupId(group.id)}
                    >
                      <span className="agent-trace-page__list-agent">{agent}</span>
                      <span className="agent-trace-page__list-label">{label}</span>
                      <span className="agent-trace-page__list-time">
                        {ts ? new Date(ts).toLocaleTimeString() : "—"}
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </Card>

        <Card className="detail-section agent-trace-page__detail-card">
          <h3 className="agent-trace-page__list-title">Detail (full body)</h3>
          {selectedGroup ? (
            <TraceGroupView group={selectedGroup} />
          ) : (
            <p className="text-muted text-sm">Select a timeline entry.</p>
          )}
          {selectedGroup?.kind === "pair" ? (
            <details className="agent-trace-raw">
              <summary>Raw JSON (both sides)</summary>
              <pre className="agent-trace-payload__body">
                {prettyJson({
                  request: selectedGroup.request.body,
                  response: selectedGroup.response.body,
                })}
              </pre>
            </details>
          ) : null}
        </Card>
      </div>
    </div>
  );
}
