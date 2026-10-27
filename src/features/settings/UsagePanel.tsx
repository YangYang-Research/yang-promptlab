import { useCallback, useEffect, useState } from "react";

import {
  Button,
  Card,
  ContentToolbar,
  ListCard,
  Pagination,
} from "@/shared/components";
import { toAppError } from "@/shared/errors";
import { usePageSizePreference } from "@/shared/hooks/usePageSizePreference";
import { usePaginatedList } from "@/shared/hooks/usePaginatedList";
import { useViewPreference } from "@/shared/hooks/useViewPreference";
import { listModels, type ModelEntryDto } from "@/shared/ipc/models";
import {
  getRuntimeTokenUsage,
  resetRuntimeTokenUsage,
  type AgentTokenUsageRow,
  type ModelTokenUsageRow,
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

type UsageRow = {
  key: string;
  title: string;
  id: string;
  note?: string | null;
  inputTokens: number;
  outputTokens: number;
  calls: number;
};

export function UsagePanel({
  backendConnected,
  refreshKey = 0,
  onLoadingChange,
}: UsagePanelProps) {
  const [usage, setUsage] = useState<TokenUsageSnapshot | null>(null);
  const [registry, setRegistry] = useState<ModelEntryDto[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [resetting, setResetting] = useState(false);

  const load = useCallback(async () => {
    if (!backendConnected) {
      setUsage(null);
      setRegistry([]);
      onLoadingChange?.(false);
      return;
    }
    onLoadingChange?.(true);
    setError(null);
    try {
      const [snapshot, listed] = await Promise.all([
        getRuntimeTokenUsage(),
        listModels().catch(() => [] as ModelEntryDto[]),
      ]);
      setUsage(snapshot);
      setRegistry(listed);
    } catch (err) {
      setError(toAppError(err).message);
      setUsage(null);
      setRegistry([]);
    } finally {
      onLoadingChange?.(false);
    }
  }, [backendConnected, onLoadingChange]);

  useEffect(() => {
    void load();
  }, [load, refreshKey]);

  const resetTokens = useCallback(async () => {
    if (!window.confirm("Reset lifetime token usage for every agent and model?")) return;
    setResetting(true);
    setError(null);
    try {
      setUsage(await resetRuntimeTokenUsage());
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setResetting(false);
    }
  }, []);

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

      <UsageBreakdown
        title="By Agent"
        preferenceKey="settings-usage-agents"
        identityHeader="Agent / Sub-Agent"
        rows={(usage?.agents ?? []).map(agentUsageRow)}
      />
      <UsageBreakdown
        title="By Model"
        preferenceKey="settings-usage-models"
        identityHeader="Model"
        rows={modelUsageRows(usage?.models ?? [], registry)}
      />
      <div className="settings-section__actions">
        <Button variant="danger" disabled={resetting} onClick={() => void resetTokens()}>
          {resetting ? "Resetting…" : "Reset tokens"}
        </Button>
      </div>
    </div>
  );
}

function agentUsageRow(row: AgentTokenUsageRow): UsageRow {
  return {
    key: row.agentId,
    title: row.label,
    id: row.agentId,
    note: row.note,
    inputTokens: row.inputTokens,
    outputTokens: row.outputTokens,
    calls: row.calls,
  };
}

function modelUsageRows(rows: ModelTokenUsageRow[], registry: ModelEntryDto[]): UsageRow[] {
  const names = new Map(registry.map((model) => [model.id, model.version || model.name]));
  return rows.map((row) => ({
    key: row.modelId,
    title: names.get(row.modelId) || row.modelId,
    id: row.modelId,
    inputTokens: row.inputTokens,
    outputTokens: row.outputTokens,
    calls: row.calls,
  }));
}

function UsageBreakdown({
  title,
  preferenceKey,
  identityHeader,
  rows,
}: {
  title: string;
  preferenceKey: string;
  identityHeader: string;
  rows: UsageRow[];
}) {
  const [viewMode, setViewMode] = useViewPreference(preferenceKey, "table");
  const [pageSize, setPageSize] = usePageSizePreference(preferenceKey, 5);
  const { page, setPage, pagination } = usePaginatedList(rows, pageSize);

  return (
    <Card>
      <div className="card__header-row">
        <div className="settings-usage__heading">
          <h3 className="card__title">{title}</h3>
        </div>
        <ContentToolbar
          pageSize={pageSize}
          onPageSizeChange={setPageSize}
          viewMode={viewMode}
          onViewModeChange={setViewMode}
        />
      </div>

      {viewMode === "table" ? (
        <div className="settings-usage__table-wrap">
          <table className="settings-usage__table">
            <thead>
              <tr>
                <th scope="col">{identityHeader}</th>
                <th scope="col">Input</th>
                <th scope="col">Output</th>
                <th scope="col">Calls</th>
              </tr>
            </thead>
            <tbody>
              {pagination.items.map((row) => (
                <tr key={row.key}>
                  <td>
                    <UsageIdentity row={row} />
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
              key={row.key}
              title={row.title}
              metadata={[
                ...(row.title === row.id ? [] : [{ label: "ID", value: row.id }]),
                { label: "Input", value: formatTokens(row.inputTokens) },
                { label: "Output", value: formatTokens(row.outputTokens) },
                { label: "Calls", value: formatTokens(row.calls) },
                ...(row.note ? [{ label: "Note", value: row.note }] : []),
              ]}
            />
          ))}
        </div>
      )}

      {rows.length > 0 ? (
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
  );
}

function UsageIdentity({ row }: { row: UsageRow }) {
  return (
    <div className="settings-usage__agent">
      <span className="settings-usage__agent-label">{row.title}</span>
      {row.title !== row.id ? (
        <span className="settings-usage__agent-id mono">{row.id}</span>
      ) : null}
      {row.note ? <span className="settings-usage__agent-note">{row.note}</span> : null}
    </div>
  );
}
