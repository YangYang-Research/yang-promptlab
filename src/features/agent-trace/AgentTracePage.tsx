import { useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { yazgChatSessionIds } from "@/features/yazg/yazgChatSession";
import {
  Card,
  ContentToolbar,
  DataTable,
  EmptyState,
  ListCard,
  PageHeader,
  Pagination,
  RefreshButton,
  StatusBadge,
} from "@/shared/components";
import { toAppError } from "@/shared/errors";
import { usePageSizePreference } from "@/shared/hooks/usePageSizePreference";
import { usePaginatedList } from "@/shared/hooks/usePaginatedList";
import { useViewPreference } from "@/shared/hooks/useViewPreference";
import {
  deleteAgentTraceSession,
  listAgentTraceSessions,
  listAgentTraces,
  type AgentTraceSummaryDto,
} from "@/shared/ipc/agentTrace";

import {
  conversationLabel,
  formatExecutionTime,
  formatTokenCount,
  formatTraceTime,
  shortId,
  stateLabel,
} from "./traceFormat";

type TraceRow = AgentTraceSummaryDto & { [key: string]: unknown };

export function AgentTracePage() {
  const { backendConnected, ui } = useAppStore();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const sessionFilter = searchParams.get("session")?.trim() || null;
  const traceRedirect = searchParams.get("trace")?.trim() || null;

  const [traces, setTraces] = useState<AgentTraceSummaryDto[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [viewMode, setViewMode] = useViewPreference("agent-trace");
  const [pageSize, setPageSize] = usePageSizePreference("agent-trace");

  useEffect(() => {
    if (traceRedirect) {
      navigate(`/agent-trace/${encodeURIComponent(traceRedirect)}`, {
        replace: true,
      });
    }
  }, [navigate, traceRedirect]);

  async function refresh() {
    if (!backendConnected) return;
    setLoading(true);
    setError(null);
    try {
      const liveIds = yazgChatSessionIds();
      const sessions = await listAgentTraceSessions({
        experiment: "yazg",
        limit: 200,
      });
      const orphans = sessions.filter((row) => !liveIds.has(row.sessionId));
      if (orphans.length > 0) {
        await Promise.all(
          orphans.map((row) =>
            deleteAgentTraceSession(row.sessionId).catch(() => null),
          ),
        );
      }

      const rows = await listAgentTraces({
        experiment: "yazg",
        sessionId: sessionFilter,
        limit: 200,
      });
      setTraces(
        rows.filter((row) => {
          const sessionId = row.sessionId?.trim();
          if (!sessionId) return true;
          return liveIds.has(sessionId);
        }),
      );
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, [backendConnected, sessionFilter]);

  useEffect(() => {
    if (!backendConnected) return;
    const timer = window.setInterval(() => {
      void refresh();
    }, 5000);
    return () => window.clearInterval(timer);
  }, [backendConnected, sessionFilter]);

  const filtered = useMemo(() => {
    const q = ui.searchQuery.toLowerCase().trim();
    if (!q) return traces;
    return traces.filter((trace) => {
      const conv = conversationLabel(trace.sessionId);
      return (
        trace.id.toLowerCase().includes(q) ||
        (trace.sessionId ?? "").toLowerCase().includes(q) ||
        conv.id.toLowerCase().includes(q) ||
        conv.title.toLowerCase().includes(q) ||
        stateLabel(trace.status).toLowerCase().includes(q) ||
        trace.name.toLowerCase().includes(q)
      );
    });
  }, [traces, ui.searchQuery]);

  const { page, setPage, pagination } = usePaginatedList(filtered, pageSize);
  const rows = pagination.items as TraceRow[];

  const openTrace = (traceId: string) => {
    navigate(`/agent-trace/${encodeURIComponent(traceId)}`);
  };

  const columns = useMemo(
    () => [
      {
        key: "id",
        header: "Trace ID",
        render: (trace: TraceRow) => (
          <code className="mono text-sm">{trace.id}</code>
        ),
      },
      {
        key: "sessionId",
        header: "Conversation ID",
        render: (trace: TraceRow) => {
          const conv = conversationLabel(
            typeof trace.sessionId === "string" ? trace.sessionId : null,
          );
          return <code className="mono text-sm">{conv.id}</code>;
        },
      },
      {
        key: "totalTokens",
        header: "Tokens",
        width: "90px",
        render: (trace: TraceRow) =>
          formatTokenCount(
            typeof trace.totalTokens === "number" ? trace.totalTokens : null,
          ),
      },
      {
        key: "latencyMs",
        header: "Execution Time",
        width: "120px",
        render: (trace: TraceRow) =>
          formatExecutionTime(
            typeof trace.latencyMs === "number" ? trace.latencyMs : null,
          ),
      },
      {
        key: "startedAt",
        header: "Request Time",
        width: "160px",
        render: (trace: TraceRow) =>
          formatTraceTime(
            typeof trace.startedAt === "string" ? trace.startedAt : null,
          ),
      },
      {
        key: "status",
        header: "State",
        width: "100px",
        render: (trace: TraceRow) => (
          <StatusBadge status={stateLabel(String(trace.status ?? "—"))} />
        ),
      },
    ],
    [],
  );

  return (
    <div className="page">
      <PageHeader
        title="Agent Trace"
        description="Yazg turns recorded by agenttrace — open a row for span detail."
        actions={
          <RefreshButton
            onClick={() => void refresh()}
            loading={loading}
            error={error}
            showSuccessToast={false}
          />
        }
      />

      {!backendConnected ? (
        <Card>
          <EmptyState
            title="Backend offline"
            description="Connect the desktop backend to read traces from SQLite."
          />
        </Card>
      ) : null}

      {error ? (
        <Card>
          <p className="text-danger">{error}</p>
        </Card>
      ) : null}

      {backendConnected && traces.length === 0 && !loading ? (
        <EmptyState
          title="No traces yet"
          description="Chat in Yazg to record turns. Deleted conversations are pruned here."
          action={
            <Link className="btn btn--secondary btn--sm" to="/yazg">
              Open Yazg
            </Link>
          }
        />
      ) : null}

      {backendConnected && (traces.length > 0 || loading) ? (
        <>
          <ContentToolbar
            filters={
              sessionFilter ? (
                <div className="agent-trace-page__filter-chip">
                  <span>
                    Session{" "}
                    <code className="mono">{shortId(sessionFilter, 18)}</code>
                  </span>
                  <Link
                    className="agent-trace-page__filter-clear"
                    to="/agent-trace"
                  >
                    Clear
                  </Link>
                </div>
              ) : null
            }
            pageSize={pageSize}
            onPageSizeChange={setPageSize}
            viewMode={viewMode}
            onViewModeChange={setViewMode}
          />

          {viewMode === "table" ? (
            <Card padding="none">
              <DataTable
                columns={columns}
                rows={rows}
                keyField="id"
                onRowClick={(row) => openTrace(row.id)}
                emptyMessage={
                  loading ? "Loading traces…" : "No traces match your search"
                }
                loading={loading && rows.length === 0}
              />
            </Card>
          ) : (
            <div className="list-card-grid">
              {pagination.items.map((trace) => {
                const conv = conversationLabel(trace.sessionId);
                return (
                  <ListCard
                    key={trace.id}
                    title={
                      <span className="mono" title={trace.id}>
                        {trace.id}
                      </span>
                    }
                    status={
                      <StatusBadge status={stateLabel(trace.status)} />
                    }
                    metadata={[
                      {
                        label: "Conversation ID",
                        value: (
                          <span className="mono" title={trace.sessionId ?? undefined}>
                            {conv.id}
                          </span>
                        ),
                      },
                      {
                        label: "Tokens",
                        value: formatTokenCount(trace.totalTokens),
                      },
                      {
                        label: "Execution Time",
                        value: formatExecutionTime(trace.latencyMs),
                      },
                      {
                        label: "Request Time",
                        value: formatTraceTime(trace.startedAt),
                      },
                    ]}
                    footerMeta={`State: ${stateLabel(trace.status)}`}
                    onClick={() => openTrace(trace.id)}
                  />
                );
              })}
            </div>
          )}

          {filtered.length > 0 ? (
            <Pagination
              page={page}
              totalItems={pagination.totalItems}
              rangeStart={pagination.rangeStart}
              rangeEnd={pagination.rangeEnd}
              totalPages={pagination.totalPages}
              onPageChange={setPage}
            />
          ) : null}
        </>
      ) : null}
    </div>
  );
}
