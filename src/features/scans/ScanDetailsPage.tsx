import { useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  ActionsDropdown,
  Badge,
  Button,
  Card,
  DataTable,
  EmptyState,
  PageHeader,
  ProgressBar,
  SeverityBadge,
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
import { buildScanProgressUrl, buildScanWizardUrl, isLiveScanStatus } from "@/features/scans/wizardState";
import { getScan, getTarget, resumeScan, deleteScan, type ScanDetailDto, type TargetDto } from "@/shared/ipc";
import { toAppError } from "@/shared/errors";
import { useToast } from "@/shared/notifications";
import type { DiscoveredEndpoint, Finding, ScanRun, Severity } from "@/shared/types";

const SEVERITY_ORDER: Severity[] = ["critical", "high", "medium", "low", "info"];

function isMonitorableAttackScan(scan: Pick<ScanRun, "name">): boolean {
  return scan.name.startsWith("Scan (") || scan.name.startsWith("Agent Scan (");
}

export function ScanDetailsPage() {
  const { scanId = "" } = useParams();
  const navigate = useNavigate();
  const { notify, dismiss } = useToast();
  const { scans, projects, targets, endpoints, findings, reports, actions } = useAppStore();
  const [detail, setDetail] = useState<ScanDetailDto | null>(null);
  const [targetDto, setTargetDto] = useState<TargetDto | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [exporting, setExporting] = useState<string | null>(null);
  const [resumePending, setResumePending] = useState(false);
  const [deletePending, setDeletePending] = useState(false);

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
  const recentFindings = useMemo(
    () =>
      [...scanFindings]
        .sort((a, b) => b.discoveredAt.localeCompare(a.discoveredAt))
        .slice(0, 5),
    [scanFindings],
  );
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

  async function handleResumeScan() {
    setResumePending(true);
    let pendingToastId: number | undefined;
    try {
      pendingToastId = notify("Resuming scan…", "info");
      await resumeScan(scanId);
      dismiss(pendingToastId);
      pendingToastId = undefined;
      notify("Scan resumed", "success");
      await actions.refresh();
      if (scan?.projectId && scan.targetId) {
        navigate(
          buildScanWizardUrl(scan.projectId, scan.targetId, {
            scanId: scan.id,
            step: 5,
          }),
        );
      }
    } catch (err) {
      if (pendingToastId !== undefined) dismiss(pendingToastId);
      notify(toAppError(err).message || "Failed to resume scan", "error");
    } finally {
      setResumePending(false);
    }
  }

  async function handleDeleteScan() {
    if (!scan) return;
    const confirmed = window.confirm(
      `Delete scan "${scan.name}"? This permanently removes findings and reports linked to this scan.`,
    );
    if (!confirmed) return;

    setDeletePending(true);
    try {
      await deleteScan(scanId);
      await actions.refresh();
      notify("Scan deleted", "success");
      navigate("/scans");
    } catch (err) {
      notify(toAppError(err).message || "Failed to delete scan", "error");
    } finally {
      setDeletePending(false);
    }
  }

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

  const effectiveStatus = status?.status ?? scan?.status ?? "pending";
  const showResumeScan =
    scan &&
    scan.targetId &&
    playbook &&
    isAttackScanName(scan.name) &&
    effectiveStatus === "paused";

  const showRetryScan =
    scan &&
    scan.targetId &&
    playbook &&
    isAttackScanName(scan.name) &&
    (effectiveStatus === "failed" ||
      effectiveStatus === "cancelled" ||
      effectiveStatus === "stopped");

  return (
    <div className="page scan-details">
      <PageHeader
        backTo="/scans"
        backOnly
        title={scan?.name ?? "Scan Details"}
        actions={
          <div className="page-actions">
            {showResumeScan && (
              <Button
                variant="primary"
                disabled={resumePending}
                onClick={() => void handleResumeScan()}
              >
                {resumePending ? "Resuming…" : "Resume Scan"}
              </Button>
            )}
            {showRetryScan && (
              <Button
                variant="primary"
                onClick={() =>
                  navigate(
                    buildScanWizardUrl(scan!.projectId, scan!.targetId ?? undefined, {
                      scanId: scan!.id,
                      step: 4,
                    }),
                  )
                }
              >
                Retry Scan
              </Button>
            )}
            {showViewScan && (
              <Button
                variant={showRetryScan || showResumeScan ? "secondary" : "primary"}
                onClick={() =>
                  navigate(
                    buildScanProgressUrl(scan!.projectId, scan!.id, scan!.targetId),
                  )
                }
              >
                View Scan Progress
              </Button>
            )}
            <ActionsDropdown
              disabled={deletePending}
              items={[
                {
                  id: "delete",
                  label: "Delete Scan",
                  tone: "danger",
                  disabled: deletePending,
                  onClick: () => void handleDeleteScan(),
                },
              ]}
            />
          </div>
        }
      />

      {error && (
        <Card>
          <p className="text-danger">{error}</p>
        </Card>
      )}

      {(targetUrl || project) && (
        <p className="scan-details__lead">
          {targetUrl ? <span className="mono">{targetUrl}</span> : null}
          {targetUrl && project ? <span className="scan-details__lead-sep"> · </span> : null}
          {project ? (
            <Link to={`/projects/${project.id}`} className="link">
              {project.name}
            </Link>
          ) : null}
        </p>
      )}

      <section className="scan-details__overview" aria-label="Scan overview">
        <Card className="detail-section scan-details__execution">
          <div className="detail-section__header">
            <div>
              <h2 className="detail-section__title">Execution</h2>
              <p className="detail-section__hint">Runtime status and pipeline progress</p>
            </div>
            <StatusBadge status={(status?.status ?? scan?.status ?? "pending") as never} />
          </div>

          {status && (status.status === "running" || status.status === "paused") && (
            <ProgressBar value={status.progress_percent} label="Scan progress" size="sm" />
          )}

          <div className="detail-summary-grid detail-summary-grid--metrics scan-details__metrics">
            <ScanSummaryStat
              label="Findings"
              value={scanFindings.length}
              accent={scanFindings.length > 0 ? "active" : undefined}
            />
            <ScanSummaryStat
              label="Progress"
              value={status ? `${status.progress_percent}%` : "—"}
              valueSm
            />
            <ScanSummaryStat
              label="Duration"
              value={formatDurationMs(durationMs)}
              valueSm
            />
            <ScanSummaryStat
              label="Pipeline"
              value={
                status
                  ? `${status.completed}/${status.total || "—"}`
                  : "—"
              }
              valueSm
            />
          </div>

          <div className="scan-details__subsection">
            <h3 className="scan-details__subsection-title">Timeline</h3>
            <div className="detail-section__body">
              <DetailRow
                label="Started"
                value={formatTimestamp(scan?.startedAt ?? scan?.createdAt ?? null)}
              />
              <DetailRow label="Finished" value={formatTimestamp(scan?.completedAt ?? null)} />
            </div>
          </div>
        </Card>

        <Card className="detail-section scan-details__context">
          <h2 className="detail-section__title">Scan context</h2>
          <div className="detail-section__body">
            <DetailRow label="Scan name" value={scan?.name ?? "—"} />
            <DetailRow label="Scan ID" value={<code>{scanId}</code>} mono />
            <DetailRow
              label="Project"
              value={
                project ? (
                  <Link to={`/projects/${project.id}`} className="link">
                    {project.name}
                  </Link>
                ) : (
                  "—"
                )
              }
            />
            <DetailRow label="Authentication" value={extractAuthType(descriptor)} />
            <DetailRow
              label="Auth configuration"
              value={extractAuthSummary(descriptor)}
            />
          </div>
        </Card>
      </section>

      {playbook && (
        <section className="scan-details__config" aria-label="Attack configuration">
          <Card className="detail-section">
            <div className="detail-section__header">
              <div>
                <h2 className="detail-section__title">Attack configuration</h2>
                <p className="detail-section__hint">
                  {profileLabel(playbook.profile)} profile · {playbook.categories.length}{" "}
                  {playbook.categories.length === 1 ? "category" : "categories"}
                </p>
              </div>
            </div>

            <div className="scan-details__config-grid">
              <div className="scan-details__plan">
                <h3 className="scan-details__subsection-title">Attack plan</h3>
                <div className="detail-section__body">
                  <DetailRow label="Profile" value={profileLabel(playbook.profile)} />
                  <DetailRow
                    label="Categories"
                    value={playbook.categories.join(", ") || "—"}
                  />
                  <DetailRow
                    label="Est. requests"
                    value={estimates?.requests.toLocaleString() ?? "—"}
                  />
                  <DetailRow label="Est. runtime" value={estimates?.runtime ?? "—"} />
                </div>
                {selectedTests.length > 0 && (
                  <div className="scan-details__subsection">
                    <h3 className="scan-details__subsection-title">Selected tests</h3>
                    <ul className="detail-list scan-details__test-list">
                      {selectedTests.map((test) => (
                        <li key={test}>{test}</li>
                      ))}
                    </ul>
                  </div>
                )}
              </div>

              <div className="scan-details__endpoints-summary">
                <h3 className="scan-details__subsection-title">Endpoints</h3>
                <div className="detail-summary-grid detail-summary-grid--metrics">
                  <ScanSummaryStat
                    label="Selected"
                    value={selectedEndpoints.length}
                  />
                  <ScanSummaryStat label="Manual" value={manualEndpoints.length} />
                </div>
              </div>
            </div>

            {selectedEndpoints.length > 0 && (
              <div className="scan-details__subsection">
                <h3 className="scan-details__subsection-title">Endpoint inventory</h3>
                <EndpointTable endpoints={selectedEndpoints} showSelected={false} />
              </div>
            )}
          </Card>
        </section>
      )}

      <section className="scan-details__insights" aria-label="Findings and reports">
        <Card className="detail-section scan-details__findings-panel">
          <div className="detail-section__header">
            <div>
              <h2 className="detail-section__title">Findings</h2>
              <p className="detail-section__hint">
                {scanFindings.length === 0
                  ? "No vulnerabilities recorded for this scan."
                  : `${scanFindings.length} finding${scanFindings.length === 1 ? "" : "s"} across severity levels`}
              </p>
            </div>
            {scanFindings.length > 0 ? (
              <Link to={`/findings?scanId=${encodeURIComponent(scanId)}`} className="link">
                View all
              </Link>
            ) : null}
          </div>

          <div className="detail-summary-grid detail-summary-grid--severity">
            {SEVERITY_ORDER.map((severity) => (
              <ScanSummaryStat
                key={severity}
                severity={severity}
                value={severityCounts.get(severity) ?? 0}
              />
            ))}
          </div>

          {recentFindings.length > 0 && (
            <div className="scan-details__subsection">
              <h3 className="scan-details__subsection-title">Recent findings</h3>
              <ul className="detail-list">
                {recentFindings.map((finding) => (
                  <li key={finding.id} className="detail-list-row">
                    <SeverityBadge severity={finding.severity} />
                    <span className="detail-list-row__title">{finding.title}</span>
                    <span className="text-muted text-sm detail-list-row__meta">
                      {formatTimestamp(finding.discoveredAt)}
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </Card>

        <Card className="detail-section scan-details__reports-panel">
          <div className="detail-section__header">
            <div>
              <h2 className="detail-section__title">Reports</h2>
              <p className="detail-section__hint">
                {scanReports.length === 0
                  ? "Export findings when the scan completes."
                  : `${scanReports.length} generated report${scanReports.length === 1 ? "" : "s"}`}
              </p>
            </div>
          </div>

          {scanReports.length === 0 ? (
            <p className="text-muted text-sm">No reports generated yet.</p>
          ) : (
            <ul className="detail-list">
              {scanReports.map((report) => (
                <li key={report.id} className="detail-list-row detail-list-row--reports">
                  <span className="detail-list-row__title">{report.title}</span>
                  <Badge variant="muted">{report.format.toUpperCase()}</Badge>
                  <span className="text-muted text-sm detail-list-row__meta">
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
            <div className="scan-details__subsection scan-details__report-actions">
              <h3 className="scan-details__subsection-title">Generate report</h3>
              <div className="scan-details__export-actions">
                {(["html", "pdf", "sarif"] as ReportExportFormat[]).map((format) => (
                  <Button
                    key={format}
                    variant="secondary"
                    size="sm"
                    disabled={scanFindings.length === 0 || exporting !== null}
                    onClick={() => void handleExport(format)}
                  >
                    {exporting === format ? "Generating…" : reportExportLabel(format)}
                  </Button>
                ))}
              </div>
            </div>
          )}
        </Card>
      </section>
    </div>
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

function ScanSummaryStat({
  label,
  value,
  severity,
  accent,
  valueSm,
}: {
  label?: string;
  value: number | string;
  severity?: Severity;
  accent?: "active";
  valueSm?: boolean;
}) {
  return (
    <div
      className={[
        "summary-stat",
        accent === "active" ? "summary-stat--active" : "",
      ]
        .filter(Boolean)
        .join(" ")}
    >
      {severity ? (
        <SeverityBadge severity={severity} />
      ) : (
        <span className="summary-stat__label">{label}</span>
      )}
      <span
        className={[
          "summary-stat__value",
          valueSm ? "summary-stat__value--sm" : "",
        ]
          .filter(Boolean)
          .join(" ")}
      >
        {value}
      </span>
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
