import { useEffect, useMemo, useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import {
  Badge,
  Button,
  Card,
  DataTable,
  EmptyState,
  PageHeader,
  StatusBadge,
} from "@/shared/components";
import { useToast } from "@/shared/notifications";
import type { DiscoveredEndpoint } from "@/shared/types";
import type { DiscoveryStatsDto } from "@/shared/ipc";

function kindVariant(kind: string): "default" | "success" | "warning" | "danger" | "info" | "muted" {
  switch (kind) {
    case "ai_endpoint":
      return "danger";
    case "openapi":
    case "graphql":
      return "warning";
    case "rest_api":
      return "info";
    default:
      return "muted";
  }
}

const endpointColumns = [
  {
    key: "url",
    header: "Endpoint",
    render: (e: DiscoveredEndpoint) => (
      <div>
        <span className="mono text-sm">{e.url}</span>
        {e.evidence && <div className="text-muted text-sm">{e.evidence}</div>}
      </div>
    ),
  },
  {
    key: "kind",
    header: "Kind",
    width: "130px",
    render: (e: DiscoveredEndpoint) => (
      <Badge variant={kindVariant(e.kind)}>{e.kind.replace(/_/g, " ")}</Badge>
    ),
  },
  {
    key: "method",
    header: "Method",
    width: "90px",
    render: (e: DiscoveredEndpoint) => e.method ?? "—",
  },
  {
    key: "confidence",
    header: "Confidence",
    width: "100px",
    render: (e: DiscoveredEndpoint) => `${Math.round(e.confidence * 100)}%`,
  },
];

export function DiscoveryPage() {
  const { targets, scans, endpoints, projects, loading, error, actions } = useAppStore();
  const { notify } = useToast();
  const [targetId, setTargetId] = useState("");
  const [running, setRunning] = useState(false);
  const [lastStats, setLastStats] = useState<DiscoveryStatsDto | null>(null);

  useEffect(() => {
    if (!targetId && targets.length > 0) {
      setTargetId(targets[0].id);
    }
  }, [targets, targetId]);

  const targetName = (id: string | null) =>
    id ? targets.find((t) => t.id === id)?.name ?? "—" : "—";
  const projectName = (id: string) => projects.find((p) => p.id === id)?.name ?? "—";

  const endpointsByScan = useMemo(() => {
    const map = new Map<string, DiscoveredEndpoint[]>();
    for (const e of endpoints) {
      const list = map.get(e.scanId) ?? [];
      list.push(e);
      map.set(e.scanId, list);
    }
    return map;
  }, [endpoints]);

  // Show discovery runs newest first (attack scans are shown on the Attacks page).
  const runs = useMemo(
    () =>
      scans
        .filter((s) => s.name.startsWith("Discovery:"))
        .sort((a, b) => b.createdAt.localeCompare(a.createdAt)),
    [scans],
  );

  const selectedTarget = targets.find((t) => t.id === targetId) ?? null;

  async function handleStartScan() {
    if (!targetId || running) return;
    setRunning(true);
    setLastStats(null);
    try {
      const result = await actions.runDiscovery(targetId);
      setLastStats(result.stats);
      notify(
        `Discovery complete — ${result.stats.endpoint_count} endpoint(s) found`,
        "success",
      );
    } catch (err) {
      const message = err instanceof Error ? err.message : "Discovery failed";
      notify(message, "error");
    } finally {
      setRunning(false);
    }
  }

  const noTargets = targets.length === 0;

  return (
    <div className="page">
      <PageHeader
        title="Discovery"
        description="Attack surface enumeration — crawl, API, OpenAPI, AI fingerprinting"
        actions={
          <div className="discovery-controls">
            <select
              className="input"
              value={targetId}
              onChange={(e) => setTargetId(e.target.value)}
              disabled={noTargets || running}
            >
              {noTargets && <option value="">No targets — add one first</option>}
              {targets.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.name} ({t.url || "no url"})
                </option>
              ))}
            </select>
            <Button
              variant="primary"
              onClick={handleStartScan}
              disabled={noTargets || running || !targetId}
            >
              {running ? "Scanning…" : "Start Scan"}
            </Button>
          </div>
        }
      />

      {error && (
        <Card>
          <p className="text-danger">Failed to load discovery data: {error}</p>
        </Card>
      )}

      {running && (
        <Card className="discovery-progress">
          <div className="discovery-progress__row">
            <div className="page-loader__spinner" />
            <div>
              <strong>Scanning {selectedTarget?.name ?? "target"}…</strong>
              <p className="text-muted text-sm">
                Crawling {selectedTarget?.url} and probing for APIs, OpenAPI, GraphQL and AI endpoints.
              </p>
            </div>
          </div>
        </Card>
      )}

      {lastStats && !running && (
        <Card>
          <h3 className="card__title">Last run</h3>
          <div className="discovery-card__stats">
            <div>
              <span className="discovery-card__stat-value">{lastStats.endpoint_count}</span>
              <span className="discovery-card__stat-label">Endpoints</span>
            </div>
            <div>
              <span className="discovery-card__stat-value">{lastStats.pages_fetched}</span>
              <span className="discovery-card__stat-label">Pages</span>
            </div>
            <div>
              <span className="discovery-card__stat-value">{lastStats.probes_sent}</span>
              <span className="discovery-card__stat-label">Probes</span>
            </div>
            <div>
              <span className="discovery-card__stat-value">{lastStats.duration_ms} ms</span>
              <span className="discovery-card__stat-label">Duration</span>
            </div>
          </div>
        </Card>
      )}

      {runs.length === 0 && !running ? (
        <EmptyState
          title={loading ? "Loading…" : "No discovery runs yet"}
          description={
            noTargets
              ? "Add a target on the Targets page, then start a scan."
              : "Select a target and click Start Scan to enumerate its attack surface."
          }
        />
      ) : (
        <div className="discovery-runs">
          {runs.map((run) => {
            const runEndpoints = endpointsByScan.get(run.id) ?? [];
            return (
              <Card key={run.id} className="discovery-card">
                <div className="discovery-card__header">
                  <div>
                    <h3 className="discovery-card__title">{targetName(run.targetId)}</h3>
                    <p className="text-muted text-sm">
                      {projectName(run.projectId)} ·{" "}
                      {run.completedAt
                        ? `Completed ${new Date(run.completedAt).toLocaleString()}`
                        : run.startedAt
                          ? `Started ${new Date(run.startedAt).toLocaleString()}`
                          : `Created ${new Date(run.createdAt).toLocaleString()}`}
                    </p>
                  </div>
                  <StatusBadge status={run.status} />
                </div>

                <div className="discovery-card__stats">
                  <div>
                    <span className="discovery-card__stat-value">{runEndpoints.length}</span>
                    <span className="discovery-card__stat-label">Endpoints</span>
                  </div>
                  <div>
                    <span className="discovery-card__stat-value">
                      {runEndpoints.filter((e) => e.kind === "ai_endpoint").length}
                    </span>
                    <span className="discovery-card__stat-label">AI endpoints</span>
                  </div>
                </div>

                {runEndpoints.length > 0 && (
                  <DataTable
                    columns={endpointColumns}
                    rows={runEndpoints}
                    keyField="id"
                    emptyMessage="No endpoints"
                  />
                )}
              </Card>
            );
          })}
        </div>
      )}
    </div>
  );
}
