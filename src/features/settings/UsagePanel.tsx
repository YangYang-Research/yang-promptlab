import { useCallback, useEffect, useState } from "react";

import {
  Badge,
  Card,
  ContentToolbar,
  ListCard,
  Pagination,
} from "@/shared/components";
import { toAppError } from "@/shared/errors";
import { usePageSizePreference } from "@/shared/hooks/usePageSizePreference";
import { usePaginatedList } from "@/shared/hooks/usePaginatedList";
import { useViewPreference } from "@/shared/hooks/useViewPreference";
import {
  getRuntimeTokenUsage,
  type AgentTokenUsageRow,
  type TokenUsageSnapshot,
} from "@/shared/ipc/runtime";

function formatTokens(value: number): string {
  return new Intl.NumberFormat().format(value);
}

type UsagePanelProps = {
  backendConnected: boolean;
  /** Bump to reload usage from the shared Settings refresh control. */
  refreshKey?: number;
  onLoadingChange?: (loading: boolean) => void;
};

export function UsagePanel({
  backendConnected,
  refreshKey = 0,
  onLoadingChange,
}: UsagePanelProps) {
  const [usage, setUsage] = useState<TokenUsageSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [viewMode, setViewMode] = useViewPreference("settings-usage-agents", "table");
  const [pageSize, setPageSize] = usePageSizePreference("settings-usage-agents", 5);

  const agents = usage?.agents ?? [];
  const { page, setPage, pagination } = usePaginatedList(agents, pageSize);

  const load = useCallback(async () => {
    if (!backendConnected) {
      setUsage(null);
      onLoadingChange?.(false);
      return;
    }
    onLoadingChange?.(true);
    setError(null);
    try {
      setUsage(await getRuntimeTokenUsage());
    } catch (err) {
      setError(toAppError(err).message);
      setUsage(null);
    } finally {
      onLoadingChange?.(false);
    }
  }, [backendConnected, onLoadingChange]);

  useEffect(() => {
    void load();
  }, [load, refreshKey]);

  if (!backendConnected) {
    return (
      <Card>
        <p className="text-muted text-sm">
          Connect to the Tauri backend to view AI Runtime token usage.
        </p>
      </Card>
    );
  }

  return (
    <div className="settings-usage">
      <Card>
        <div className="settings-usage__totals">
          <div className="settings-usage__stat">
            <span className="settings-usage__stat-label">Total input</span>
            <strong className="settings-usage__stat-value">
              {formatTokens(usage?.totalInputTokens ?? 0)}
            </strong>
          </div>
          <div className="settings-usage__stat">
            <span className="settings-usage__stat-label">Total output</span>
            <strong className="settings-usage__stat-value">
              {formatTokens(usage?.totalOutputTokens ?? 0)}
            </strong>
          </div>
          <div className="settings-usage__stat">
            <span className="settings-usage__stat-label">Completions</span>
            <strong className="settings-usage__stat-value">
              {formatTokens(usage?.totalCalls ?? 0)}
            </strong>
          </div>
        </div>
        {error ? <p className="text-danger text-sm">{error}</p> : null}
      </Card>

      <Card>
        <div className="card__header-row">
          <div className="settings-usage__heading">
            <h3 className="card__title">By agent</h3>
            <Badge variant="muted">{agents.length} agents</Badge>
          </div>
        </div>

        <ContentToolbar
          pageSize={pageSize}
          onPageSizeChange={setPageSize}
          viewMode={viewMode}
          onViewModeChange={setViewMode}
        />

        {viewMode === "table" ? (
          <div className="settings-usage__table-wrap">
            <table className="settings-usage__table">
              <thead>
                <tr>
                  <th scope="col">Agent / sub-agent</th>
                  <th scope="col">Input</th>
                  <th scope="col">Output</th>
                  <th scope="col">Calls</th>
                </tr>
              </thead>
              <tbody>
                {pagination.items.map((row) => (
                  <tr key={row.agentId}>
                    <td>
                      <AgentIdentity row={row} />
                    </td>
                    <td className="mono">{formatTokens(row.inputTokens)}</td>
                    <td className="mono">{formatTokens(row.outputTokens)}</td>
                    <td className="mono">{formatTokens(row.calls)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="list-card-grid settings-usage__list">
            {pagination.items.map((row) => (
              <ListCard
                key={row.agentId}
                title={row.label}
                metadata={[
                  { label: "ID", value: row.agentId },
                  { label: "Input", value: formatTokens(row.inputTokens) },
                  { label: "Output", value: formatTokens(row.outputTokens) },
                  { label: "Calls", value: formatTokens(row.calls) },
                  ...(row.note ? [{ label: "Note", value: row.note }] : []),
                ]}
              />
            ))}
          </div>
        )}

        {agents.length > 0 ? (
          <Pagination
            page={page}
            totalItems={pagination.totalItems}
            rangeStart={pagination.rangeStart}
            rangeEnd={pagination.rangeEnd}
            totalPages={pagination.totalPages}
            onPageChange={setPage}
          />
        ) : null}
      </Card>
    </div>
  );
}

function AgentIdentity({ row }: { row: AgentTokenUsageRow }) {
  return (
    <div className="settings-usage__agent">
      <span className="settings-usage__agent-label">{row.label}</span>
      <span className="settings-usage__agent-id mono">{row.agentId}</span>
      {row.note ? <span className="settings-usage__agent-note">{row.note}</span> : null}
    </div>
  );
}
