import { useEffect, useMemo, useState } from "react";
import { Link, useParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { Badge, Button, Card, DataTable, EmptyState, PageHeader, StatusBadge } from "@/shared/components";
import { endpointSourceLabel } from "@/features/scans/discoveryPhases";
import {
  formatDurationMs,
  formatTimestamp,
  isManualEndpoint,
  mapScanDetailToRun,
} from "@/features/scans/scanDetailsHelpers";
import {
  countDiscoveryStats,
  parseAttackPlaybook,
  parseDiscoveryPlaybook,
} from "@/features/scans/scanPlaybook";
import { getScan, type ScanDetailDto } from "@/shared/ipc";
import type { DiscoveredEndpoint } from "@/shared/types";

export function DiscoveryDetailsPage() {
  const { scanId = "" } = useParams();
  const { scans, projects, targets, endpoints, actions } = useAppStore();
  const [detail, setDetail] = useState<ScanDetailDto | null>(null);
  const [selectedEndpointIds, setSelectedEndpointIds] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const scan = scans.find((item) => item.id === scanId) ?? (detail ? mapScanDetailToRun(detail) : null);
  const project = projects.find((item) => item.id === scan?.projectId);
  const target = targets.find((item) => item.id === scan?.targetId);
  const runEndpoints = useMemo(
    () => endpoints.filter((endpoint) => endpoint.scanId === scanId),
    [endpoints, scanId],
  );
  const manualEndpoints = runEndpoints.filter((endpoint) =>
    isManualEndpoint(endpoint.kind, endpoint.sourceUrl),
  );
  const stats = countDiscoveryStats(runEndpoints);
  const playbookStats = parseDiscoveryPlaybook(detail?.playbook);

  useEffect(() => {
    if (!scanId) return;
    let cancelled = false;
    setLoading(true);
    setError(null);

    void getScan(scanId)
      .then((dto) => {
        if (!cancelled) setDetail(dto);
      })
      .catch((err) => {
        if (!cancelled) setError(err instanceof Error ? err.message : "Failed to load discovery run");
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [scanId]);

  useEffect(() => {
    if (!scan?.targetId) return;
    let cancelled = false;
    const attackScans = scans.filter(
      (item) => item.targetId === scan.targetId && item.name.startsWith("Scan ("),
    );

    void Promise.all(attackScans.map((item) => getScan(item.id)))
      .then((details) => {
        if (cancelled) return;
        const selected = new Set<string>();
        for (const item of details) {
          const playbook = parseAttackPlaybook(item.playbook);
          playbook?.endpointIds.forEach((id) => selected.add(id));
        }
        setSelectedEndpointIds(selected);
      })
      .catch(() => {
        if (!cancelled) setSelectedEndpointIds(new Set());
      });

    return () => {
      cancelled = true;
    };
  }, [scan?.targetId, scans]);

  const columns = [
    {
      key: "method",
      header: "Method",
      width: "90px",
      render: (row: DiscoveredEndpoint) => row.method ?? "—",
    },
    {
      key: "url",
      header: "Endpoint",
      render: (row: DiscoveredEndpoint) => <span className="mono text-sm">{row.url}</span>,
    },
    {
      key: "confidence",
      header: "Confidence",
      width: "100px",
      render: (row: DiscoveredEndpoint) => `${Math.round(row.confidence * 100)}%`,
    },
    {
      key: "source",
      header: "Source",
      width: "110px",
      render: (row: DiscoveredEndpoint) => (
        <Badge variant={isManualEndpoint(row.kind, row.sourceUrl) ? "info" : "muted"}>
          {endpointSourceLabel(row.kind, row.sourceUrl)}
        </Badge>
      ),
    },
    {
      key: "selected",
      header: "Selected For Scan",
      width: "130px",
      render: (row: DiscoveredEndpoint) =>
        selectedEndpointIds.has(row.id) ? (
          <Badge variant="success">Yes</Badge>
        ) : (
          <span className="text-muted">No</span>
        ),
    },
  ];

  if (!scanId) {
    return (
      <div className="page">
        <EmptyState title="Discovery not found" description="Missing discovery run identifier." />
      </div>
    );
  }

  if (loading && !scan && !detail) {
    return (
      <div className="page">
        <PageHeader title="Discovery Details" description="Loading discovery run…" />
      </div>
    );
  }

  return (
    <div className="page">
      <PageHeader
        title="Discovery Details"
        description={`${project?.name ?? "Project"} · ${target?.url ?? target?.name ?? "Target"}`}
        actions={
          <div className="discovery-controls">
            <Button variant="ghost" onClick={() => void actions.refresh()}>
              Refresh
            </Button>
            <Link to="/discovery">
              <Button variant="secondary">Back to Discovery</Button>
            </Link>
          </div>
        }
      />

      {error && (
        <Card>
          <p className="text-danger">{error}</p>
        </Card>
      )}

      <Card className="detail-section">
        <div className="detail-section__body">
          <div className="detail-row">
            <span className="detail-row__label">Project</span>
            <span className="detail-row__value">{project?.name ?? "—"}</span>
          </div>
          <div className="detail-row">
            <span className="detail-row__label">Target</span>
            <span className="detail-row__value mono">{target?.url ?? "—"}</span>
          </div>
          <div className="detail-row">
            <span className="detail-row__label">Discovery timestamp</span>
            <span className="detail-row__value">
              {formatTimestamp(scan?.completedAt ?? scan?.createdAt ?? null)}
            </span>
          </div>
          <div className="detail-row">
            <span className="detail-row__label">Status</span>
            <span className="detail-row__value">
              {scan ? <StatusBadge status={scan.status} /> : "—"}
            </span>
          </div>
        </div>
      </Card>

      <Card className="detail-section">
        <h2 className="detail-section__title">Endpoint table</h2>
        <DataTable columns={columns} rows={runEndpoints} keyField="id" emptyMessage="No endpoints discovered" />
      </Card>

      {manualEndpoints.length > 0 && (
        <Card className="detail-section">
          <h2 className="detail-section__title">Manual endpoints</h2>
          <DataTable
            columns={columns.filter((column) => column.key !== "selected")}
            rows={manualEndpoints}
            keyField="id"
            emptyMessage="No manual endpoints"
          />
        </Card>
      )}

      <Card className="detail-section">
        <h2 className="detail-section__title">Discovery statistics</h2>
        <div className="discovery-card__stats">
          <Stat label="Endpoints found" value={playbookStats?.endpointCount ?? stats.total} />
          <Stat label="AI endpoints found" value={stats.ai} />
          <Stat label="GraphQL endpoints" value={stats.graphql} />
          <Stat label="OpenAPI endpoints" value={stats.openapi} />
          <Stat label="JavaScript files" value={stats.javascript} />
          <Stat
            label="Duration"
            value={formatDurationMs(playbookStats?.durationMs ?? null)}
          />
        </div>
      </Card>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string | number }) {
  return (
    <div>
      <span className="discovery-card__stat-value">{value}</span>
      <span className="discovery-card__stat-label">{label}</span>
    </div>
  );
}
