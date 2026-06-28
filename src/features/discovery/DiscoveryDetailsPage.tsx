import { useEffect, useMemo, useState } from "react";
import { useParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { Badge, Card, DataTable, EmptyState, PageHeader, RefreshButton, StatusBadge } from "@/shared/components";
import { endpointSourceLabel } from "@/features/scans/discoveryPhases";
import { endpointTypeLabel, platformLabel } from "@/features/scans/fingerprintPlan";
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
import type { EndpointAttackRecommendationDto } from "@/shared/ipc/client";
import type { DiscoveredEndpoint } from "@/shared/types";

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

  const analyzedEndpoints = runEndpoints.filter((e) => e.metadata);
  const aggregateSummary = useMemo(
    () => aggregateMetadata(analyzedEndpoints),
    [analyzedEndpoints],
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
      key: "framework",
      header: "Framework",
      width: "140px",
      render: (row: DiscoveredEndpoint) => {
        const framework = row.metadata?.fingerprint.framework ?? row.aiFramework;
        return framework ? (
          <Badge variant="info">{platformLabel(framework)}</Badge>
        ) : (
          <span className="text-muted">—</span>
        );
      },
    },
    {
      key: "endpointType",
      header: "Type",
      width: "120px",
      render: (row: DiscoveredEndpoint) =>
        row.endpointType ? endpointTypeLabel(row.endpointType) : "—",
    },
    {
      key: "risk",
      header: "Risk",
      width: "70px",
      render: (row: DiscoveredEndpoint) => row.riskScore ?? "—",
    },
    {
      key: "confidence",
      header: "Confidence",
      width: "100px",
      render: (row: DiscoveredEndpoint) =>
        `${Math.round((row.metadataConfidence ?? row.confidence) * 100)}%`,
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

      {aggregateSummary && (
        <Card className="detail-section">
          <h2 className="detail-section__title">AI Discovery Summary</h2>
          <div className="fingerprint-summary">
            <div className="fingerprint-summary__score">
              <span className="fingerprint-summary__value">
                {Math.round(aggregateSummary.confidence * 100)}%
              </span>
              <span className="fingerprint-summary__label">Confidence</span>
            </div>
            <div className="fingerprint-summary__breakdown">
              <MetadataGroup title="Frameworks" items={aggregateSummary.frameworks} />
              <MetadataGroup title="Endpoint Types" items={aggregateSummary.endpointTypes} />
              <MetadataGroup title="Technologies" items={aggregateSummary.technologies} />
              <MetadataGroup title="Transports" items={aggregateSummary.transports} />
            </div>
          </div>

          {aggregateSummary.attackRecommendations.length > 0 && (
            <div className="fingerprint-recommendations">
              <h3 className="card__title">Attack Recommendations</h3>
              <ul className="fingerprint-recommendation-list">
                {aggregateSummary.attackRecommendations.map((rec) => (
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

      {analyzedEndpoints.length > 0 && (
        <Card className="detail-section">
          <h2 className="detail-section__title">Endpoint Metadata</h2>
          {analyzedEndpoints.map((endpoint) => (
            <EndpointMetadataPanel key={endpoint.id} endpoint={endpoint} />
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

function MetadataGroup({ title, items }: { title: string; items: string[] }) {
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

function EndpointMetadataPanel({ endpoint }: { endpoint: DiscoveredEndpoint }) {
  const meta = endpoint.metadata;
  if (!meta) return null;

  const caps = meta.capabilities;
  const capLabels = [
    caps.supportsStreaming && "Streaming",
    caps.supportsTools && "Tools",
    caps.supportsVision && "Vision",
    caps.supportsEmbedding && "Embedding",
    caps.supportsAgent && "Agent",
    caps.supportsMemory && "Memory",
  ].filter(Boolean) as string[];

  return (
    <div className="fingerprint-endpoint">
      <div className="fingerprint-endpoint__header">
        <span className="mono text-sm">
          {endpoint.method ?? "GET"} {endpoint.url}
        </span>
        <Badge variant="info">{`${Math.round((endpoint.metadataConfidence ?? 0) * 100)}%`}</Badge>
      </div>
      <div className="fingerprint-endpoint__grid">
        <MetadataGroup
          title="Framework"
          items={[platformLabel(meta.fingerprint.framework || endpoint.aiFramework || "unknown")]}
        />
        <MetadataGroup
          title="Type"
          items={[endpointTypeLabel(endpoint.endpointType ?? meta.classification.endpointType)]}
        />
        <MetadataGroup title="Capabilities" items={capLabels} />
      </div>
      {(endpoint.attackRecommendations?.length ?? 0) > 0 && (
        <p className="text-muted text-sm">
          Recommended:{" "}
          {endpoint.attackRecommendations!
            .map((r) => r.category.replaceAll("_", " "))
            .join(", ")}
        </p>
      )}
    </div>
  );
}

type AggregateMetadata = {
  confidence: number;
  frameworks: string[];
  endpointTypes: string[];
  technologies: string[];
  transports: string[];
  attackRecommendations: EndpointAttackRecommendationDto[];
};

function aggregateMetadata(endpoints: DiscoveredEndpoint[]): AggregateMetadata | null {
  const metas = endpoints.map((e) => e.metadata).filter(Boolean);
  if (metas.length === 0) return null;

  const unique = (items: string[]) => [...new Set(items.filter(Boolean))].sort();

  const frameworks = unique(
    endpoints.map(
      (e) => e.metadata?.fingerprint.framework ?? e.aiFramework ?? "",
    ).map((f) => platformLabel(f)),
  );
  const endpointTypes = unique(
    endpoints.map((e) => endpointTypeLabel(e.endpointType ?? e.metadata?.classification.endpointType ?? "")),
  );
  const technologies = unique(metas.flatMap((m) => m!.fingerprint.technologies ?? []));
  const transports = unique(metas.flatMap((m) => m!.schema.transport ?? []));

  const confidence = Math.max(
    ...endpoints.map((e) => e.metadataConfidence ?? e.metadata?.fingerprint.confidence ?? 0),
  );

  const recMap = new Map<string, EndpointAttackRecommendationDto>();
  for (const endpoint of endpoints) {
    for (const rec of endpoint.attackRecommendations ?? []) {
      const existing = recMap.get(rec.category);
      if (!existing || rec.priority < existing.priority) {
        recMap.set(rec.category, rec);
      }
    }
  }
  const attackRecommendations = [...recMap.values()].sort((a, b) => a.priority - b.priority);

  return {
    confidence,
    frameworks,
    endpointTypes,
    technologies,
    transports,
    attackRecommendations,
  };
}
