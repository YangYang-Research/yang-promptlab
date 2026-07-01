import { useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  Badge,
  Button,
  Card,
  DataTable,
  EmptyState,
  PageHeader,
  ProgressBar,
  StatusBadge,
} from "@/shared/components";
import {
  exportStoredReport,
  generateAndExportScanReport,
  reportExportLabel,
  type ReportExportFormat,
} from "@/features/reports/reportDownloads";
import { endpointSourceLabel } from "@/features/scans/endpointHelpers";
import {
  extractAuthSummary,
  extractAuthType,
  extractTargetUrl,
  formatDurationMs,
  formatTimestamp,
  isManualEndpoint,
  mapScanDetailToRun,
} from "@/features/scans/scanDetailsHelpers";
import {
  estimateAttackPlan,
  isAttackScanName,
  listSelectedTests,
  parseAttackPlaybook,
  profileLabel,
} from "@/features/scans/scanPlaybook";
import { mergeScanStatus, useScanStatuses } from "@/features/scans/useScanStatuses";
import { buildScanWizardUrl } from "@/features/scans/wizardState";
import { getScan, getTarget, type ScanDetailDto, type TargetDto } from "@/shared/ipc";
import type { DiscoveredEndpoint, Finding, ScanRun, Severity } from "@/shared/types";

const SEVERITY_ORDER: Severity[] = ["critical", "high", "medium", "low", "info"];

function isLiveScanStatus(status: string | undefined): boolean {
  return status === "running" || status === "paused" || status === "pending";
}

function isMonitorableAttackScan(scan: Pick<ScanRun, "name">): boolean {
  return scan.name.startsWith("Scan (") || scan.name.startsWith("Agent Scan (");
}

export function ScanDetailsPage() {
  const { scanId = "" } = useParams();
  const navigate = useNavigate();
  const { scans, projects, targets, endpoints, findings, reports, actions } = useAppStore();
  const [detail, setDetail] = useState<ScanDetailDto | null>(null);
  const [targetDto, setTargetDto] = useState<TargetDto | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [exporting, setExporting] = useState<string | null>(null);

  const scan = scans.find((item) => item.id === scanId) ?? (detail ? mapScanDetailToRun(detail) : null);
  const project = projects.find((item) => item.id === scan?.projectId);
  const target = targets.find((item) => item.id === scan?.targetId);

  useEffect(() => {
    if (!scanId) return;
    let cancelled = false;
    setLoading(true);
    setError(null);

    void getScan(scanId)
      .then(async (dto) => {
        if (cancelled) return;
        setDetail(dto);
        if (dto.target_id) {
          const targetDetail = await getTarget(dto.target_id);
          if (!cancelled) setTargetDto(targetDetail);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : "Failed to load scan");
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [scanId]);

  const playbook = parseAttackPlaybook(detail?.playbook);
  const scanFindings = useMemo(
    () => findings.filter((finding) => finding.scanId === scanId),
    [findings, scanId],
  );
  const scanReports = useMemo(
    () => reports.filter((report) => report.scanId === scanId),
    [reports, scanId],
  );

  const selectedEndpoints = useMemo(() => {
    if (!playbook) return [];
    const ids = new Set(playbook.endpointIds);
    return endpoints.filter((endpoint) => ids.has(endpoint.id));
  }, [endpoints, playbook]);

  const manualEndpoints = selectedEndpoints.filter((endpoint) =>
    isManualEndpoint(endpoint.kind, endpoint.sourceUrl),
  );

  const severityCounts = useMemo(() => countSeverities(scanFindings), [scanFindings]);
  const statuses = useScanStatuses(scanId ? [scanId] : [], Boolean(scanId));
  const live = statuses.get(scanId);
  const status = scan
    ? mergeScanStatus(scanId, scan.status, live, scanFindings.length)
    : null;

  const estimates = playbook
    ? estimateAttackPlan(
        playbook.endpointIds.length,
        playbook.profile,
        playbook.categories,
        playbook.disabledTests,
      )
    : null;

  const selectedTests = playbook
    ? listSelectedTests(playbook.categories, playbook.disabledTests)
    : [];

  const durationMs =
    scan?.startedAt && scan?.completedAt
      ? new Date(scan.completedAt).getTime() - new Date(scan.startedAt).getTime()
      : null;

  async function handleExport(format: ReportExportFormat) {
    setExporting(format);
    try {
      if (scan) {
        await generateAndExportScanReport(scan.projectId, scanId, format);
        await actions.refresh();
      }
    } finally {
      setExporting(null);
    }
  }

  if (!scanId) {
    return (
      <div className="page">
        <PageHeader title="Scan Details" backTo="/scans" backOnly />
        <EmptyState title="Scan not found" description="Missing scan identifier." />
      </div>
    );
  }

  if (loading && !scan && !detail) {
    return (
      <div className="page">
        <PageHeader title="Scan Details" backTo="/scans" backOnly description="Loading scan configuration…" />
      </div>
    );
  }

  if (error && !scan && !detail) {
    return (
      <div className="page">
        <PageHeader title="Scan Details" backTo="/scans" backOnly />
        <EmptyState title="Scan not found" description={error} />
      </div>
    );
  }

  const descriptor = targetDto?.descriptor ?? null;
  const targetUrl = target?.url ?? extractTargetUrl(descriptor);
  const showViewScan =
    scan &&
    scan.targetId &&
    isMonitorableAttackScan(scan) &&
    isLiveScanStatus(status?.status ?? scan.status);

  return (
    <div className="page">
      <PageHeader
        backTo="/scans"
        backOnly
        title={scan?.name ?? "Scan Details"}
        actions={
          showViewScan ? (
            <Button
              variant="primary"
              onClick={() =>
                navigate(
                  buildScanWizardUrl(scan.projectId, scan.targetId ?? undefined, {
                    scanId: scan.id,
                    step: 5,
                  }),
                )
              }
            >
              View Scan
            </Button>
          ) : undefined
        }
      />

      {error && (
        <Card>
          <p className="text-danger">{error}</p>
        </Card>
      )}

      <div className="detail-sections">
        <DetailSection title="Scan">
          <DetailRow label="Name" value={scan?.name ?? "—"} />
          <DetailRow label="Scan ID" value={<code>{scanId}</code>} mono />
        </DetailSection>

        <DetailSection title="Project">
          <DetailRow label="Project name" value={project?.name ?? "—"} />
        </DetailSection>

        <DetailSection title="Target">
          <DetailRow label="Target URL" value={targetUrl || "—"} mono />
          <DetailRow label="Authentication type" value={extractAuthType(descriptor)} />
          <DetailRow label="Authentication configuration" value={extractAuthSummary(descriptor)} />
        </DetailSection>

        {playbook && (
          <>
            <DetailSection title="Endpoints">
              <DetailRow label="Selected endpoints" value={String(selectedEndpoints.length)} />
              <DetailRow label="Manual endpoints" value={String(manualEndpoints.length)} />
              {selectedEndpoints.length > 0 && (
                <EndpointTable endpoints={selectedEndpoints} showSelected={false} />
              )}
            </DetailSection>

            <DetailSection title="Attack Plan">
              <DetailRow label="Profile" value={profileLabel(playbook.profile)} />
              <DetailRow label="Selected categories" value={playbook.categories.join(", ") || "—"} />
              <DetailRow label="Estimated requests" value={estimates?.requests.toLocaleString() ?? "—"} />
              <DetailRow label="Estimated runtime" value={estimates?.runtime ?? "—"} />
              {selectedTests.length > 0 && (
                <ul className="detail-list">
                  {selectedTests.map((test) => (
                    <li key={test}>{test}</li>
                  ))}
                </ul>
              )}
            </DetailSection>
          </>
        )}

        <DetailSection title="Execution">
          <DetailRow
            label="Status"
            value={<StatusBadge status={(status?.status ?? scan?.status ?? "pending") as never} />}
          />
          <DetailRow label="Started" value={formatTimestamp(scan?.startedAt ?? scan?.createdAt ?? null)} />
          <DetailRow label="Finished" value={formatTimestamp(scan?.completedAt ?? null)} />
          <DetailRow label="Duration" value={formatDurationMs(durationMs)} />
          <DetailRow
            label="Progress"
            value={
              status
                ? `${status.progress_percent}% (${status.completed}/${status.total || "—"} tests)`
                : "—"
            }
          />
          {status && (status.status === "running" || status.status === "paused") && (
            <ProgressBar value={status.progress_percent} label="Scan progress" size="sm" />
          )}
        </DetailSection>

        <DetailSection title="Results">
          <DetailRow label="Findings count" value={String(scanFindings.length)} />
          {SEVERITY_ORDER.map((severity) => (
            <DetailRow
              key={severity}
              label={severity.charAt(0).toUpperCase() + severity.slice(1)}
              value={String(severityCounts.get(severity) ?? 0)}
            />
          ))}
          <Link to={`/findings?scanId=${encodeURIComponent(scanId)}`}>View findings →</Link>
        </DetailSection>

        <DetailSection title="Reports">
          {scanReports.length === 0 ? (
            <p className="text-muted">No reports generated yet.</p>
          ) : (
            <ul className="detail-list">
              {scanReports.map((report) => (
                <li key={report.id} className="detail-report-row">
                  <span>
                    {report.title} · {report.format.toUpperCase()} ·{" "}
                    {formatTimestamp(report.createdAt)}
                  </span>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => void exportStoredReport(report.id)}
                  >
                    Download
                  </Button>
                </li>
              ))}
            </ul>
          )}
          {playbook && isAttackScanName(scan?.name ?? "") && (
            <div className="detail-report-actions">
              {(["html", "pdf", "sarif"] as ReportExportFormat[]).map((format) => (
                <Button
                  key={format}
                  variant="secondary"
                  size="sm"
                  disabled={scanFindings.length === 0 || exporting !== null}
                  onClick={() => void handleExport(format)}
                >
                  {exporting === format ? "Generating…" : `Generate ${reportExportLabel(format)}`}
                </Button>
              ))}
            </div>
          )}
        </DetailSection>
      </div>
    </div>
  );
}

function DetailSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <Card className="detail-section">
      <h2 className="detail-section__title">{title}</h2>
      <div className="detail-section__body">{children}</div>
    </Card>
  );
}

function DetailRow({
  label,
  value,
  mono,
}: {
  label: string;
  value: React.ReactNode;
  mono?: boolean;
}) {
  return (
    <div className="detail-row">
      <span className="detail-row__label">{label}</span>
      <span className={`detail-row__value${mono ? " mono" : ""}`}>{value}</span>
    </div>
  );
}

function EndpointTable({
  endpoints,
  showSelected,
  selectedIds,
}: {
  endpoints: DiscoveredEndpoint[];
  showSelected?: boolean;
  selectedIds?: Set<string>;
}) {
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
    ...(showSelected
      ? [
          {
            key: "selected",
            header: "Selected",
            width: "90px",
            render: (row: DiscoveredEndpoint) =>
              selectedIds?.has(row.id) ? <Badge variant="success">Yes</Badge> : "No",
          },
        ]
      : []),
  ];

  return (
    <DataTable columns={columns} rows={endpoints} keyField="id" emptyMessage="No endpoints" />
  );
}

function countSeverities(findings: Finding[]): Map<Severity, number> {
  const counts = new Map<Severity, number>();
  for (const severity of SEVERITY_ORDER) counts.set(severity, 0);
  for (const finding of findings) {
    counts.set(finding.severity, (counts.get(finding.severity) ?? 0) + 1);
  }
  return counts;
}
