import { useCallback, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  ActionsDropdown,
  type ActionsDropdownItem,
  AuthTypeBadge,
  Badge,
  Button,
  Card,
  ContentToolbar,
  DataTable,
  EmptyState,
  IconFolder,
  IconProgress,
  IconTrash,
  ListCard,
  PageHeader,
  PageLoadingSkeleton,
  Pagination,
  StatusBadge,
  TargetScanStatusBadge,
} from "@/shared/components";
import { formatTimestamp } from "@/features/scans/scanDetailsHelpers";
import { targetDisplayType } from "@/features/scans/targetProfile";
import {
  buildScanProgressUrl,
  buildScanWizardUrl,
  peekWizardSession,
  wizardResumeInputFromSession,
} from "@/features/scans/wizardState";
import {
  buildTargetScanContext,
  formatTargetTimestamp,
} from "@/shared/targetScanContext";
import { resolveTargetScanAction } from "@/shared/targetScanAction";
import { usePageSizePreference } from "@/shared/hooks/usePageSizePreference";
import { usePaginatedList } from "@/shared/hooks/usePaginatedList";
import { useViewPreference } from "@/shared/hooks/useViewPreference";
import { useToast } from "@/shared/notifications";
import type { ScanRun } from "@/shared/types";

function isAttackScan(scan: ScanRun): boolean {
  return scan.name.startsWith("Scan (") || scan.name.startsWith("Agent Scan (");
}

function isLiveScan(scan: ScanRun): boolean {
  return scan.status === "running" || scan.status === "paused" || scan.status === "pending";
}

export function TargetDetailsPage() {
  const { targetId = "" } = useParams();
  const navigate = useNavigate();
  const { targets, projects, scans, findings, loading, actions } = useAppStore();
  const { notify } = useToast();
  const [deleting, setDeleting] = useState(false);
  const [pageSize, setPageSize] = usePageSizePreference("target-details-scans");
  const [viewMode, setViewMode] = useViewPreference("target-details-scans");

  const target = targets.find((item) => item.id === targetId);
  const project = projects.find((item) => item.id === target?.projectId);

  const targetScans = useMemo(
    () =>
      scans
        .filter((scan) => scan.targetId === targetId)
        .sort((a, b) => b.createdAt.localeCompare(a.createdAt)),
    [scans, targetId],
  );

  const attackScans = useMemo(
    () => targetScans.filter(isAttackScan),
    [targetScans],
  );

  const findingsByScan = useMemo(() => {
    const map = new Map<string, number>();
    for (const finding of findings) {
      map.set(finding.scanId, (map.get(finding.scanId) ?? 0) + 1);
    }
    return map;
  }, [findings]);

  const { page, setPage, pagination } = usePaginatedList(attackScans, pageSize);

  const scanColumns = useMemo(
    () => [
      {
        key: "name",
        header: "Scan",
        render: (scan: ScanRun) => (
          <div>
            <strong>{scan.name}</strong>
            <div className="mono text-sm text-muted">{scan.id}</div>
          </div>
        ),
      },
      {
        key: "status",
        header: "Status",
        width: "120px",
        render: (scan: ScanRun) => <StatusBadge status={scan.status} />,
      },
      {
        key: "findings",
        header: "Findings",
        width: "90px",
        render: (scan: ScanRun) => findingsByScan.get(scan.id) ?? 0,
      },
      {
        key: "started",
        header: "Started",
        width: "180px",
        render: (scan: ScanRun) => formatTimestamp(scan.startedAt ?? scan.createdAt),
      },
    ],
    [findingsByScan],
  );

  const scanContext = useMemo(
    () => (target ? buildTargetScanContext(target.id, scans) : null),
    [target, scans],
  );

  const wizardSession = useMemo(() => peekWizardSession(), []);

  const scanAction = useMemo(() => {
    if (!target) return null;
    return resolveTargetScanAction(
      target.id,
      target.projectId,
      scans,
      wizardSession ? wizardResumeInputFromSession(wizardSession) : null,
    );
  }, [target, scans, wizardSession]);

  const runningScan = useMemo(
    () => attackScans.find(isLiveScan) ?? null,
    [attackScans],
  );

  const handleDeleteTarget = useCallback(async () => {
    if (!target) return;
    const hasActiveScan = scans.some(
      (scan) => scan.targetId === target.id && isLiveScan(scan),
    );
    const confirmed = window.confirm(
      hasActiveScan
        ? `Delete target "${target.name}"? Any active scan will be stopped. This cannot be undone.`
        : `Delete target "${target.name}"? This cannot be undone.`,
    );
    if (!confirmed) return;

    setDeleting(true);
    try {
      await actions.deleteTarget(target.id);
      notify(`Target "${target.name}" deleted`, "success");
      navigate(`/projects/${target.projectId}`);
    } catch {
      notify("Failed to delete target", "error");
    } finally {
      setDeleting(false);
    }
  }, [actions, navigate, notify, scans, target]);

  const startContinueSetup = useCallback(() => {
    if (!target) return;
    if (scanAction?.kind === "setup") {
      navigate(
        buildScanWizardUrl(target.projectId, target.id, {
          step: scanAction.step,
          scanId: scanAction.scanId,
        }),
      );
      return;
    }
    navigate(buildScanWizardUrl(target.projectId, target.id, { step: 3 }));
  }, [navigate, scanAction, target]);

  const startNewScan = useCallback(() => {
    if (!target) return;
    navigate(buildScanWizardUrl(target.projectId, target.id, { step: 2 }));
  }, [navigate, target]);

  const actionItems = useMemo((): ActionsDropdownItem[] => {
    if (!target || !scanAction) return [];

    const items: ActionsDropdownItem[] = [];

    if (scanAction.kind === "view_scan") {
      items.push({
        id: "view-progress",
        label: "View Scan Progress",
        icon: <IconProgress />,
        onClick: () =>
          navigate(buildScanProgressUrl(target.projectId, scanAction.scanId, target.id)),
      });
    }

    // Keep New Scan available when primary CTA is Progress / Retry — not while
    // the target is still Pending (Continue Setup is the only path).
    if (
      target.status !== "pending" &&
      (scanAction.kind === "view_scan" || scanAction.kind === "retry")
    ) {
      items.push({
        id: "new-scan",
        label: "New Scan",
        onClick: startNewScan,
      });
    }

    if (project) {
      items.push({
        id: "view-project",
        label: "View Project",
        icon: <IconFolder />,
        onClick: () => navigate(`/projects/${project.id}`),
      });
    }

    items.push({
      id: "delete",
      label: "Delete Target",
      icon: <IconTrash />,
      tone: "danger",
      disabled: deleting,
      onClick: () => void handleDeleteTarget(),
    });

    return items;
  }, [deleting, handleDeleteTarget, navigate, project, scanAction, startNewScan, target]);

  if (!target && !loading) {
    return (
      <div className="page">
        <PageHeader title="Target Details" backTo="/targets" backOnly />
        <EmptyState title="Target not found" description="This target may have been deleted." />
      </div>
    );
  }

  if (!target || !scanContext) {
    return (
      <div className="page target-details">
        <PageHeader title="Target Details" backTo="/targets" backOnly />
        <PageLoadingSkeleton />
      </div>
    );
  }

  const primaryCta =
    runningScan != null
      ? {
          label: "View Scan Progress",
          onClick: () =>
            navigate(buildScanProgressUrl(target.projectId, runningScan.id, target.id)),
        }
      : scanAction?.kind === "retry"
        ? {
            label: "Retry Scan",
            onClick: () =>
              navigate(
                buildScanWizardUrl(target.projectId, target.id, {
                  step: scanAction.step,
                  scanId: scanAction.scanId,
                }),
              ),
          }
        : scanAction?.kind === "setup" || target.status === "pending"
          ? {
              label: "Continue Setup",
              onClick: startContinueSetup,
            }
          : {
              // Completed / verified ready target — primary entry for a fresh wizard run.
              label: "New Scan",
              onClick: startNewScan,
            };

  return (
    <div className="page target-details">
      <PageHeader
        backTo="/targets"
        backOnly
        title={target.name}
        actions={
          <div className="page-actions">
            <Button variant="primary" onClick={primaryCta.onClick}>
              {primaryCta.label}
            </Button>
            <ActionsDropdown
              label="Target actions"
              disabled={deleting}
              items={actionItems}
            />
          </div>
        }
      />

      <section className="target-details__overview" aria-label="Target overview">
        <Card className="detail-section target-details__meta">
          <div className="detail-section__header">
            <h2 className="detail-section__title">Target Information</h2>
            <StatusBadge status={target.status} />
          </div>
          <div className="detail-section__body">
            <DetailRow
              label="Project"
              value={
                project ? (
                  <Link to={`/projects/${project.id}`} className="link">
                    {project.name}
                  </Link>
                ) : (
                  "Unknown project"
                )
              }
            />
            <DetailRow label="Host" value={target.name} />
            <DetailRow label="Endpoint" value={<span className="mono">{target.url}</span>} />
            <DetailRow
              label="Type"
              value={<Badge variant="info">{targetDisplayType(target)}</Badge>}
            />
            <DetailRow
              label="Authentication"
              value={<AuthTypeBadge kind={target.authKind} label={target.authType} />}
            />
          </div>
        </Card>

        <Card className="detail-section target-details__scan-panel">
          <div className="detail-section__header">
            <h2 className="detail-section__title">Scan Status</h2>
            <TargetScanStatusBadge label={scanContext.scanStatusLabel} />
          </div>
          <div className="detail-summary-grid detail-summary-grid--metrics target-details__scan-metrics">
            <div className="summary-stat">
              <span className="summary-stat__label">Attack Scans</span>
              <span className="summary-stat__value">{attackScans.length}</span>
            </div>
            <div
              className={[
                "summary-stat",
                scanContext.scanStatusLabel === "Running" ? "summary-stat--active" : "",
              ]
                .filter(Boolean)
                .join(" ")}
            >
              <span className="summary-stat__label">Last scan</span>
              <span className="summary-stat__value summary-stat__value--sm">
                {formatTargetTimestamp(scanContext.lastScanTime)}
              </span>
            </div>
          </div>
          <div className="target-details__subsection">
            <h3 className="target-details__subsection-title">Latest result</h3>
            <p className="target-details__result-text">{scanContext.latestScanResult}</p>
          </div>
        </Card>
      </section>

      <section className="target-details__primary" aria-label="Attack Scans">
        <Card className="detail-section target-details__scans-card">
          <div className="detail-section__header">
            <h2 className="detail-section__title">Attack Scans</h2>
          </div>

          {attackScans.length === 0 ? (
            <EmptyState
              title="No attack scans yet"
              description="Start a scan to run security tests against this target."
            />
          ) : (
            <>
              <ContentToolbar
                pageSize={pageSize}
                onPageSizeChange={setPageSize}
                viewMode={viewMode}
                onViewModeChange={setViewMode}
              />
              {viewMode === "table" ? (
                <div className="target-details__scans-table">
                  <DataTable
                    columns={scanColumns}
                    rows={pagination.items}
                    keyField="id"
                    emptyMessage="No attack scans"
                    onRowClick={(scan) => navigate(`/scans/${scan.id}`)}
                  />
                </div>
              ) : (
                <div className="list-card-grid">
                  {pagination.items.map((scan) => (
                    <ListCard
                      key={scan.id}
                      title={scan.name}
                      status={<StatusBadge status={scan.status} />}
                      metadata={[
                        {
                          label: "Scan ID",
                          value: <span className="mono text-sm">{scan.id}</span>,
                        },
                        {
                          label: "Findings",
                          value: findingsByScan.get(scan.id) ?? 0,
                        },
                        {
                          label: "Started",
                          value: formatTimestamp(scan.startedAt ?? scan.createdAt),
                        },
                      ]}
                      onClick={() => navigate(`/scans/${scan.id}`)}
                    />
                  ))}
                </div>
              )}
              <Pagination
                page={page}
                totalItems={pagination.totalItems}
                rangeStart={pagination.rangeStart}
                rangeEnd={pagination.rangeEnd}
                totalPages={pagination.totalPages}
                onPageChange={setPage}
              />
            </>
          )}
        </Card>
      </section>
    </div>
  );
}

function DetailRow({
  label,
  value,
  capitalize = false,
}: {
  label: string;
  value: React.ReactNode;
  capitalize?: boolean;
}) {
  return (
    <div className="detail-row">
      <span className="detail-row__label">{label}</span>
      <span className={`detail-row__value ${capitalize ? "detail-row__value--cap" : ""}`}>
        {value}
      </span>
    </div>
  );
}
