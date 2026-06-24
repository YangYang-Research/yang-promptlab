import { useEffect, useMemo, useState } from "react";
import { useParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { Badge, Card, DataTable, EmptyState, PageHeader, RefreshButton, StatusBadge } from "@/shared/components";
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
import type { DiscoveredEndpoint, EndpointFingerprint } from "@/shared/types";

export function DiscoveryDetailsPage() {
  const { scanId = "" } = useParams();
  const { scans, projects, targets, endpoints, actions, loading: storeLoading, error: storeError } = useAppStore();
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

  const fingerprintedEndpoints = runEndpoints.filter((e) => e.fingerprint);
  const aggregateFingerprint = useMemo(
    () => aggregateFingerprints(fingerprintedEndpoints),
    [fingerprintedEndpoints],
  );

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
      key: "fingerprint",
      header: "Stack",
      width: "140px",
      render: (row: DiscoveredEndpoint) =>
        row.fingerprint ? (
          <Badge variant="info">
            {row.fingerprint.primaryProvider ?? row.fingerprint.technologies[0]?.name ?? "AI"}
          </Badge>
        ) : (
          <span className="text-muted">—</span>
        ),
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
        <PageHeader title="Discovery Details" backTo="/discovery" backOnly />
        <EmptyState title="Discovery not found" description="Missing discovery run identifier." />
      </div>
    );
  }

  if (loading && !scan && !detail) {
    return (
      <div className="page">
        <PageHeader title="Discovery Details" backTo="/discovery" backOnly description="Loading discovery run…" />
      </div>
    );
  }

  return (
    <div className="page">
      <PageHeader
        backTo="/discovery"
        backOnly
        title="Discovery Details"
        actions={
          <RefreshButton
            loading={storeLoading}
            error={storeError}
            onClick={() => void actions.refresh()}
          />
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

      {aggregateFingerprint && (
        <Card className="detail-section">
          <h2 className="detail-section__title">AI Fingerprint</h2>
          <div className="fingerprint-summary">
            <div className="fingerprint-summary__score">
              <span className="fingerprint-summary__value">
                {Math.round(aggregateFingerprint.confidence * 100)}%
              </span>
              <span className="fingerprint-summary__label">Confidence</span>
            </div>
            <div className="fingerprint-summary__breakdown">
              <FingerprintGroup
                title="Technologies"
                items={aggregateFingerprint.technologies.map((t) => t.name)}
              />
              <FingerprintGroup
                title="Agent Frameworks"
                items={aggregateFingerprint.agentFrameworks.map((f) => f.name)}
              />
              <FingerprintGroup
                title="AI Components"
                items={aggregateFingerprint.aiComponents.map((c) => c.name)}
              />
              <FingerprintGroup
                title="Methods"
                items={aggregateFingerprint.methodsUsed.map((m) => m.replaceAll("_", " "))}
              />
            </div>
          </div>

          {aggregateFingerprint.attackRecommendations.length > 0 && (
            <div className="fingerprint-recommendations">
              <h3 className="card__title">Attack Recommendations</h3>
              <ul className="fingerprint-recommendation-list">
                {aggregateFingerprint.attackRecommendations.map((rec) => (
                  <li key={rec.category}>
                    <strong>{rec.category.replaceAll("_", " ")}</strong>
                    <span className="text-muted text-sm"> — {rec.reason}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </Card>
      )}

      {fingerprintedEndpoints.length > 0 && (
        <Card className="detail-section">
          <h2 className="detail-section__title">Technology Breakdown</h2>
          {fingerprintedEndpoints.map((endpoint) => (
            <EndpointFingerprintPanel key={endpoint.id} endpoint={endpoint} />
          ))}
        </Card>
      )}

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

function FingerprintGroup({ title, items }: { title: string; items: string[] }) {
  if (items.length === 0) return null;
  return (
    <div className="fingerprint-group">
      <h4 className="fingerprint-group__title">{title}</h4>
      <div className="fingerprint-group__tags">
        {items.map((item) => (
          <Badge key={item} variant="muted">
            {item}
          </Badge>
        ))}
      </div>
    </div>
  );
}

function EndpointFingerprintPanel({ endpoint }: { endpoint: DiscoveredEndpoint }) {
  const fp = endpoint.fingerprint;
  if (!fp) return null;
  return (
    <div className="fingerprint-endpoint">
      <div className="fingerprint-endpoint__header">
        <span className="mono text-sm">{endpoint.url}</span>
        <Badge variant="info">{`${Math.round(fp.confidence * 100)}%`}</Badge>
      </div>
      <div className="fingerprint-endpoint__grid">
        <FingerprintGroup title="Technologies" items={fp.technologies.map((t) => t.name)} />
        <FingerprintGroup title="Frameworks" items={fp.agentFrameworks.map((f) => f.name)} />
        <FingerprintGroup title="Components" items={fp.aiComponents.map((c) => c.name)} />
      </div>
      {fp.attackRecommendations.length > 0 && (
        <p className="text-muted text-sm">
          Recommended: {fp.attackRecommendations.map((r) => r.category.replaceAll("_", " ")).join(", ")}
        </p>
      )}
    </div>
  );
}

type AggregateFingerprint = {
  confidence: number;
  technologies: FingerprintTechnology[];
  agentFrameworks: FingerprintFramework[];
  aiComponents: FingerprintComponent[];
  attackRecommendations: FingerprintRecommendation[];
  methodsUsed: string[];
};

type FingerprintTechnology = EndpointFingerprint["technologies"][number];
type FingerprintFramework = EndpointFingerprint["agentFrameworks"][number];
type FingerprintComponent = EndpointFingerprint["aiComponents"][number];
type FingerprintRecommendation = EndpointFingerprint["attackRecommendations"][number];

function aggregateFingerprints(endpoints: DiscoveredEndpoint[]): AggregateFingerprint | null {
  const fps = endpoints.map((e) => e.fingerprint).filter(Boolean) as EndpointFingerprint[];
  if (fps.length === 0) return null;

  const mergeByName = <T extends { name: string; confidence: number }>(items: T[]) => {
    const map = new Map<string, T>();
    for (const item of items) {
      const existing = map.get(item.name);
      if (!existing || item.confidence > existing.confidence) {
        map.set(item.name, item);
      }
    }
    return [...map.values()].sort((a, b) => b.confidence - a.confidence);
  };

  const technologies = mergeByName(fps.flatMap((fp) => fp.technologies));
  const agentFrameworks = mergeByName(fps.flatMap((fp) => fp.agentFrameworks));
  const aiComponents = mergeByName(fps.flatMap((fp) => fp.aiComponents));
  const methodsUsed = [...new Set(fps.flatMap((fp) => fp.methodsUsed))].sort();
  const confidence = Math.max(...fps.map((fp) => fp.confidence));

  const recMap = new Map<string, FingerprintRecommendation>();
  for (const fp of fps) {
    for (const rec of fp.attackRecommendations) {
      const existing = recMap.get(rec.category);
      if (!existing || rec.priority < existing.priority) {
        recMap.set(rec.category, rec);
      }
    }
  }
  const attackRecommendations = [...recMap.values()].sort((a, b) => a.priority - b.priority);

  return {
    confidence,
    technologies,
    agentFrameworks,
    aiComponents,
    attackRecommendations,
    methodsUsed,
  };
}
