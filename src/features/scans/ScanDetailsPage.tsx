import { useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  ActionsDropdown,
  type ActionsDropdownItem,
  AuthTypeBadge,
  Badge,
  Button,
  Card,
  EmptyState,
  IconDownload,
  IconPause,
  IconPlay,
  IconStop,
  IconTrash,
  IconCheck,
  PageHeader,
  Pagination,
  ProgressBar,
  scanOpenActionIcon,
  SeverityBadge,
  StatusBadge,
} from "@/shared/components";
import {
  generateAndExportScanReport,
  reportExportLabel,
  type ReportExportFormat,
} from "@/features/reports/reportDownloads";
import {
  buildFindingsByCategory,
  FindingsByCategoryChart,
} from "@/features/scans/FindingsByCategoryChart";
import {
  extractAuthKind,
  extractAuthType,
  extractTargetUrl,
  formatDurationMs,
  formatTimestamp,
  mapScanDetailToRun,
} from "@/features/scans/scanDetailsHelpers";
import {
  formatExecutionStrategySummary,
  attackPlanFromExecutionPlaybook,
} from "@/features/scans/attackPlan";
import { formatPayloadGenerationStrategy } from "@/features/scans/payloadStrategy";
import {
  isAttackScanName,
  listPlanCategoryGroups,
  parseAttackPlaybook,
  profileLabel,
} from "@/features/scans/scanPlaybook";
import { ScanRecommendationsPanel } from "@/features/scans/ScanRecommendationsPanel";
import { mergeScanStatus, useScanStatuses } from "@/features/scans/useScanStatuses";
import {
  buildScanProgressUrl,
  buildScanWizardUrl,
  clearWizardSessionIfReferencesScan,
  isLiveScanStatus,
  isRetryableScanStatus,
  resolveScanOpenPath,
} from "@/features/scans/wizardState";
import { usePaginatedList } from "@/shared/hooks/usePaginatedList";
import {
  getScan,
  getTarget,
  pauseScan,
  resumeScan,
  stopScan,
  deleteScan,
  type ScanDetailDto,
  type TargetDto,
} from "@/shared/ipc";
import { getTargetProfile } from "@/shared/ipc/targetProfile";
import { toAppError } from "@/shared/errors";
import { useToast } from "@/shared/notifications";
import type { Finding, ScanRun, Severity } from "@/shared/types";

import {
  buildScanConfigExport,
  downloadScanConfigJson,
} from "./scanConfigExport";

const SEVERITY_ORDER: Severity[] = ["critical", "high", "medium", "low", "info"];
const SCAN_FINDINGS_PAGE_SIZE = 5;

function isMonitorableAttackScan(scan: Pick<ScanRun, "name">): boolean {
  return scan.name.startsWith("Scan (") || scan.name.startsWith("Agent Scan (");
}

export function ScanDetailsPage() {
  const { scanId = "" } = useParams();
  const navigate = useNavigate();
  const { notify, dismiss } = useToast();
  const { scans, projects, targets, findings, actions } = useAppStore();
  const [detail, setDetail] = useState<ScanDetailDto | null>(null);
  const [targetDto, setTargetDto] = useState<TargetDto | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [exporting, setExporting] = useState<string | null>(null);
  const [controlPending, setControlPending] = useState(false);
  const [deletePending, setDeletePending] = useState(false);
  const [exportConfigPending, setExportConfigPending] = useState(false);

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
  const recentFindings = useMemo(
    () =>
      [...scanFindings].sort((a, b) => b.discoveredAt.localeCompare(a.discoveredAt)),
    [scanFindings],
  );
  const {
    page: findingsPage,
    setPage: setFindingsPage,
    pagination: findingsPagination,
  } = usePaginatedList(recentFindings, SCAN_FINDINGS_PAGE_SIZE);

  const severityCounts = useMemo(() => countSeverities(scanFindings), [scanFindings]);
  const findingsByCategory = useMemo(
    () => buildFindingsByCategory(scanFindings),
    [scanFindings],
  );
  const statuses = useScanStatuses(scanId ? [scanId] : [], Boolean(scanId));
  const live = statuses.get(scanId);
  const status = scan
    ? mergeScanStatus(scanId, scan.status, live, scanFindings.length)
    : null;
  const effectiveStatus = status?.status ?? scan?.status ?? "pending";

  const [expandedPlanCategory, setExpandedPlanCategory] = useState<string | null>(null);

  const planCategoryGroups = useMemo(
    () =>
      playbook
        ? listPlanCategoryGroups(playbook.categories, playbook.disabledTests)
        : [],
    [playbook],
  );

  const reconstructedPlan = useMemo(
    () => attackPlanFromExecutionPlaybook(detail?.playbook),
    [detail?.playbook],
  );

  const executionLabel = reconstructedPlan
    ? formatExecutionStrategySummary(reconstructedPlan)
    : playbook?.agentMode
      ? "Agentic"
      : "Sequential";

  const payloadStrategyLabel = reconstructedPlan
    ? formatPayloadGenerationStrategy(reconstructedPlan.payloadStrategy)
    : "—";

  const durationMs =
    scan?.startedAt && scan?.completedAt
      ? new Date(scan.completedAt).getTime() - new Date(scan.startedAt).getTime()
      : null;

  async function runControl(action: "pause" | "resume" | "stop") {
    setControlPending(true);
    let pendingToastId: number | undefined;
    try {
      if (action === "pause") {
        pendingToastId = notify("Pausing scan…", "info");
        await pauseScan(scanId);
        dismiss(pendingToastId);
        pendingToastId = undefined;
        notify("Scan paused", "info");
      } else if (action === "resume") {
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
        return;
      } else {
        await stopScan(scanId);
        notify("Scan stopped", "info");
      }
      await actions.refresh();
    } catch (err) {
      if (pendingToastId !== undefined) dismiss(pendingToastId);
      notify(toAppError(err).message || "Scan control failed", "error");
    } finally {
      setControlPending(false);
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
      clearWizardSessionIfReferencesScan(scanId);
      await actions.refresh();
      notify("Scan deleted", "success");
      navigate("/scans");
    } catch (err) {
      notify(toAppError(err).message || "Failed to delete scan", "error");
    } finally {
      setDeletePending(false);
    }
  }

  async function handleExportScanConfig() {
    if (!scan) return;
    setExportConfigPending(true);
    try {
      const scanDetail = detail ?? (await getScan(scan.id));
      const targetId = scanDetail.target_id ?? scan.targetId;
      if (!targetId) {
        throw new Error("Scan has no target to export.");
      }
      const [targetDetail, profile] = await Promise.all([
        targetDto && targetDto.id === targetId ? Promise.resolve(targetDto) : getTarget(targetId),
        getTargetProfile(targetId),
      ]);
      const config = buildScanConfigExport({
        scanId: scan.id,
        profile,
        descriptor: targetDetail.descriptor,
        playbook: scanDetail.playbook,
      });
      downloadScanConfigJson(config, `scan-config-${scan.id}.json`);
      notify("Scan config exported", "success");
    } catch (err) {
      notify(toAppError(err).message || "Failed to export scan config", "error");
    } finally {
      setExportConfigPending(false);
    }
  }

  function openScanAction() {
    if (!scan) return;
    navigate(resolveScanOpenPath(scan, status?.status));
  }

  function buildScanActionItems(): ActionsDropdownItem[] {
    if (!scan) return [];

    const items: ActionsDropdownItem[] = [];
    const openPath = resolveScanOpenPath(scan, status?.status);
    const detailsPath = `/scans/${scan.id}`;

    if (openPath !== detailsPath && scan.status !== "draft") {
      const openLabel = isLiveScanStatus(effectiveStatus)
        ? "View Scan Progress"
        : isRetryableScanStatus(effectiveStatus)
          ? "Retry Scan"
          : "Open Scan";
      items.push({
        id: "open",
        label: openLabel,
        icon: scanOpenActionIcon(openLabel),
        onClick: openScanAction,
      });
    }

    if (effectiveStatus === "running") {
      items.push({
        id: "pause",
        label: "Pause Scan",
        icon: <IconPause />,
        disabled: controlPending,
        onClick: () => void runControl("pause"),
      });
    }
    if (effectiveStatus === "paused") {
      items.push({
        id: "resume",
        label: "Resume Scan",
        icon: <IconPlay />,
        disabled: controlPending,
        onClick: () => void runControl("resume"),
      });
    }
    if (
      effectiveStatus === "running" ||
      effectiveStatus === "paused" ||
      effectiveStatus === "pending"
    ) {
      items.push({
        id: "stop",
        label: "Stop Scan",
        icon: <IconStop />,
        disabled: controlPending,
        onClick: () => void runControl("stop"),
      });
    }

    if (effectiveStatus === "completed") {
      items.push({
        id: "export-scan",
        label: "Export Scan",
        icon: <IconDownload />,
        disabled: exportConfigPending,
        onClick: () => void handleExportScanConfig(),
      });
    }

    items.push({
      id: "delete",
      label: "Delete Scan",
      icon: <IconTrash />,
      tone: "danger",
      disabled: deletePending,
      onClick: () => void handleDeleteScan(),
    });

    return items;
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
    isLiveScanStatus(effectiveStatus);

  const showContinueSetup = scan && scan.status === "draft";

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
    isRetryableScanStatus(effectiveStatus);

  return (
    <div className="page scan-details">
      <PageHeader
        backTo="/scans"
        backOnly
        title={scan?.name ?? "Scan Details"}
        actions={
          <div className="page-actions">
            {showContinueSetup && (
              <Button variant="primary" onClick={openScanAction}>
                Continue Setup
              </Button>
            )}
            {showResumeScan && (
              <Button
                variant="primary"
                disabled={controlPending}
                onClick={() => void runControl("resume")}
              >
                {controlPending ? "Resuming…" : "Resume Scan"}
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
                variant={
                  showRetryScan || showResumeScan || showContinueSetup ? "secondary" : "primary"
                }
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
              label="Scan actions"
              disabled={deletePending || controlPending}
              items={buildScanActionItems()}
            />
          </div>
        }
      />

      {error && (
        <Card>
          <p className="text-danger">{error}</p>
        </Card>
      )}

      <section className="scan-details__overview" aria-label="Scan overview">
        <Card className="detail-section scan-details__context">
          <h2 className="detail-section__title">Scan Information</h2>
          <div className="detail-section__body">
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
            <DetailRow label="Scan ID" value={<code>{scanId}</code>} mono />
            <DetailRow label="Scan name" value={scan?.name ?? "—"} />
            <DetailRow
              label="Endpoint"
              value={targetUrl ? <span className="mono">{targetUrl}</span> : "—"}
            />
            <DetailRow
              label="Authentication"
              value={
                <AuthTypeBadge
                  kind={extractAuthKind(descriptor)}
                  label={extractAuthType(descriptor)}
                />
              }
            />
          </div>
        </Card>

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
              label="Requests"
              value={
                status
                  ? `${status.attacks_completed ?? status.completed}/${
                      status.attacks_total || status.total || "—"
                    }`
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
      </section>

      {playbook && (
        <section className="scan-details__plan-section" aria-label="Attack Plan">
          <Card className="detail-section scan-details__plan-card">
            <header className="scan-plan__header">
              <div className="scan-plan__heading">
                <h2 className="scan-plan__title">Attack Plan</h2>
              </div>
              {playbook.agentMode ? <Badge variant="info">Agentic</Badge> : null}
            </header>

            <div className="scan-plan__metrics" role="list">
              <div className="scan-plan__metric" role="listitem">
                <span className="scan-plan__metric-label">Profile</span>
                <span className="scan-plan__metric-value">{profileLabel(playbook.profile)}</span>
              </div>
              <div className="scan-plan__metric" role="listitem">
                <span className="scan-plan__metric-label">Categories</span>
                <span className="scan-plan__metric-value">
                  {playbook.categories.length.toLocaleString()}
                </span>
              </div>
              <div className="scan-plan__metric" role="listitem">
                <span className="scan-plan__metric-label">Execution</span>
                <span className="scan-plan__metric-value">{executionLabel}</span>
              </div>
              <div className="scan-plan__metric" role="listitem">
                <span className="scan-plan__metric-label">Payload strategy</span>
                <span className="scan-plan__metric-value">{payloadStrategyLabel}</span>
              </div>
              {playbook.agentMode && playbook.maxAgentAttempts != null ? (
                <div className="scan-plan__metric" role="listitem">
                  <span className="scan-plan__metric-label">Max attempts</span>
                  <span className="scan-plan__metric-value">
                    {playbook.maxAgentAttempts.toLocaleString()}
                  </span>
                </div>
              ) : null}
            </div>

            {planCategoryGroups.length > 0 ? (
              <div className="scan-plan__block">
                <h3 className="scan-plan__block-title">Attack Category</h3>
                <div className="scan-plan__category-list">
                  {planCategoryGroups.map((group) => {
                    const expanded = expandedPlanCategory === group.categoryId;
                    return (
                      <div
                        key={group.categoryId}
                        className={`scan-plan__category${expanded ? " scan-plan__category--open" : ""}`}
                      >
                        <button
                          type="button"
                          className="scan-plan__category-toggle"
                          aria-expanded={expanded}
                          onClick={() =>
                            setExpandedPlanCategory(expanded ? null : group.categoryId)
                          }
                        >
                          <span className="scan-plan__category-name">{group.label}</span>
                          <span className="scan-plan__category-count">
                            {group.enabledCount}/{group.totalCount} tests
                          </span>
                          <span className="scan-plan__category-chevron" aria-hidden>
                            {expanded ? "▾" : "▸"}
                          </span>
                        </button>
                        {expanded ? (
                          <ul className="scan-plan__test-checklist">
                            {group.tests.map((test) => (
                              <li
                                key={test.id}
                                className={`scan-plan__test-check${
                                  test.enabled ? "" : " scan-plan__test-check--off"
                                }`}
                              >
                                <span
                                  className={`scan-plan__test-mark${
                                    test.enabled ? " scan-plan__test-mark--on" : ""
                                  }`}
                                  aria-hidden
                                >
                                  {test.enabled ? <IconCheck /> : null}
                                </span>
                                <span className="scan-plan__test-name">{test.name}</span>
                              </li>
                            ))}
                          </ul>
                        ) : null}
                      </div>
                    );
                  })}
                </div>
              </div>
            ) : null}
          </Card>
        </section>
      )}

      <section className="scan-details__insights" aria-label="Findings overview">
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
                {findingsPagination.items.map((finding) => (
                  <li key={finding.id} className="detail-list-row">
                    <SeverityBadge severity={finding.severity} />
                    <Link
                      to={`/findings/${finding.id}`}
                      className="detail-list-row__title link"
                    >
                      {finding.title}
                    </Link>
                    <span className="text-muted text-sm detail-list-row__meta">
                      {formatTimestamp(finding.discoveredAt)}
                    </span>
                  </li>
                ))}
              </ul>
              {findingsPagination.totalPages > 1 ? (
                <Pagination
                  page={findingsPage}
                  totalPages={findingsPagination.totalPages}
                  totalItems={findingsPagination.totalItems}
                  rangeStart={findingsPagination.rangeStart}
                  rangeEnd={findingsPagination.rangeEnd}
                  onPageChange={setFindingsPage}
                />
              ) : null}
            </div>
          )}
        </Card>

        <Card className="detail-section scan-details__category-panel">
          <div className="detail-section__header">
            <div>
              <h2 className="detail-section__title">By attack category</h2>
              <p className="detail-section__hint">
                {scanFindings.length === 0
                  ? "Finding counts will appear here after the scan records issues."
                  : `${findingsByCategory.length} categor${findingsByCategory.length === 1 ? "y" : "ies"} with findings`}
              </p>
            </div>
          </div>

          <FindingsByCategoryChart data={findingsByCategory} />

          {playbook && isAttackScanName(scan?.name ?? "") && (
            <div className="scan-details__subsection scan-details__report-actions">
              <h3 className="scan-details__subsection-title">Generate report</h3>
              <div className="scan-details__export-actions">
                {(["html", "pdf", "sarif", "csv"] as ReportExportFormat[]).map((format) => (
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

      {scan && isMonitorableAttackScan(scan) && (
        <section className="scan-details__recommendations" aria-label="Recommendations">
          <Card className="detail-section scan-details__recommendations-card">
            <ScanRecommendationsPanel
              scanId={scanId}
              attackCategories={playbook?.categories ?? []}
              enabled={!loading && Boolean(detail)}
              variant="details"
              projectId={scan?.projectId}
              targetId={scan?.targetId}
              revision={
                effectiveStatus === "running" || effectiveStatus === "paused"
                  ? effectiveStatus
                  : `${effectiveStatus}|${scanFindings
                      .map((f) => `${f.id}:${f.severity}:${f.title}`)
                      .sort()
                      .join(",")}`
              }
            />
          </Card>
        </section>
      )}
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

function countSeverities(findings: Finding[]): Map<Severity, number> {
  const counts = new Map<Severity, number>();
  for (const severity of SEVERITY_ORDER) counts.set(severity, 0);
  for (const finding of findings) {
    counts.set(finding.severity, (counts.get(finding.severity) ?? 0) + 1);
  }
  return counts;
}
