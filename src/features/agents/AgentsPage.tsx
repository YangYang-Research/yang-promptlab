import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { Card, EmptyState, PageHeader, RefreshButton } from "@/shared/components";
import { toAppError } from "@/shared/errors";
import { listAgents, type AgentSummaryDto } from "@/shared/ipc/agents";

function formatTemperature(value: number | null): string {
  if (value == null || !Number.isFinite(value)) return "—";
  return value.toFixed(1);
}

export function AgentsPage() {
  const { backendConnected, ui } = useAppStore();
  const navigate = useNavigate();
  const [agents, setAgents] = useState<AgentSummaryDto[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);

  useEffect(() => {
    if (!backendConnected) return;
    let cancelled = false;
    setLoading(true);
    setError(null);
    listAgents()
      .then((rows) => {
        if (!cancelled) setAgents(rows);
      })
      .catch((err) => {
        if (!cancelled) setError(toAppError(err).message);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [backendConnected, reloadKey]);

  const filtered = useMemo(() => {
    const q = ui.searchQuery.toLowerCase().trim();
    if (!q) return agents;
    return agents.filter(
      (agent) =>
        agent.displayName.toLowerCase().includes(q) ||
        agent.id.toLowerCase().includes(q) ||
        agent.description.toLowerCase().includes(q),
    );
  }, [agents, ui.searchQuery]);

  return (
    <div className="page agents-page">
      <PageHeader
        title="Agents"
        description="Registered agents, their prompts, tools, attempt budgets, and temperatures."
        actions={
          <RefreshButton
            loading={loading}
            error={error}
            disabled={!backendConnected}
            onClick={() => setReloadKey((key) => key + 1)}
          />
        }
      />

      {!backendConnected ? (
        <Card>
          <EmptyState
            title="Backend offline"
            description="Connect the desktop backend to read the agent catalog."
          />
        </Card>
      ) : null}

      {error ? (
        <Card>
          <p className="text-danger">{error}</p>
        </Card>
      ) : null}

      {backendConnected ? (
        filtered.length === 0 ? (
          <Card>
            <EmptyState
              title={loading ? "Loading agents" : "No agents"}
              description={
                loading
                  ? "Reading the agent catalog."
                  : "No registered agent matches this search."
              }
            />
          </Card>
        ) : (
          <div className="agents-card-grid">
            {filtered.map((agent) => (
              <button
                key={agent.id}
                type="button"
                className="agents-card"
                onClick={() => navigate(`/agents/${encodeURIComponent(agent.id)}`)}
              >
                <span className="agents-card__head">
                  <span className="agents-card__name">{agent.displayName}</span>
                  <span
                    className="agents-card__metrics"
                    aria-label={`Attempt ${agent.attempt ?? "none"}, Temperature ${formatTemperature(agent.temperature)}`}
                  >
                    <span className="agents-card__metric">
                      <span className="agents-card__metric-label">A</span>
                      <span className="mono agents-card__metric-value">
                        {agent.attempt ?? "—"}
                      </span>
                    </span>
                    <span className="agents-card__metric">
                      <span className="agents-card__metric-label">T</span>
                      <span className="mono agents-card__metric-value">
                        {formatTemperature(agent.temperature)}
                      </span>
                    </span>
                  </span>
                </span>
                <code className="mono agents-card__id">{agent.id}</code>
                <p className="agents-card__description">{agent.description}</p>
              </button>
            ))}
          </div>
        )
      ) : null}
    </div>
  );
}
