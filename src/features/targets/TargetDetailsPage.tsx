import { useCallback, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  ActionsDropdown,
  type ActionsDropdownItem,
  Button,
  Card,
  EmptyState,
  PageHeader,
  PageLoadingSkeleton,
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
import { resolveTargetScanAction, type TargetScanAction } from "@/shared/targetScanAction";
import { useToast } from "@/shared/notifications";
import type { ScanRun } from "@/shared/types";

function isAttackScan(scan: ScanRun): boolean {
  return scan.name.startsWith("Scan (");
}

export function TargetDetailsPage() {
  const { targetId = "" } = useParams();
  const navigate = useNavigate();
  const { targets, projects, scans, loading, actions } = useAppStore();
  const { notify } = useToast();
  const [deleting, setDeleting] = useState(false);

  const target = targets.find((item) => item.id === targetId);
  const project = projects.find((item) => item.id === target?.projectId);

  const targetScans = useMemo(
    () =>
      scans
        .filter((scan) => scan.targetId === targetId)
        .sort((a, b) => b.createdAt.localeCompare(a.createdAt)),
    [scans, targetId],
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

  const attackScans = useMemo(
    () => targetScans.filter(isAttackScan),
    [targetScans],
  );

  const recentAttackScans = useMemo(
    () => attackScans.slice(0, 5),
    [attackScans],
  );

  const handleDeleteTarget = useCallback(async () => {
    if (!target) return;
    const hasActiveScan = scans.some(
      (scan) =>
        scan.targetId === target.id &&
        (scan.status === "running" || scan.status === "paused" || scan.status === "pending"),
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

  const actionItems = useMemo((): ActionsDropdownItem[] => {
    if (!target || !scanAction) return [];

    const items: ActionsDropdownItem[] = [];

    if (scanAction.kind === "view_scan") {
      items.push({
        id: "view-scan",
        label: "View Scan Progress",
        onClick: () =>
          navigate(buildScanProgressUrl(target.projectId, scanAction.scanId, target.id)),
      });
    } else if (scanAction.kind === "view_report") {
      items.push({
        id: "view-scan",
        label: "View Scan Details",
        onClick: () => navigate(`/scans/${scanAction.scanId}`),
      });
    } else if (scanAction.kind === "retry") {
      items.push({
        id: "retry",
        label: "Retry Scan",
        onClick: () =>
          navigate(
            buildScanWizardUrl(target.projectId, target.id, {
              step: scanAction.step,
              scanId: scanAction.scanId,
            }),
          ),
      });
    } else {
      items.push({
        id: "continue-setup",
        label: "Continue Setup",
        onClick: () =>
          navigate(
            buildScanWizardUrl(target.projectId, target.id, {
              step: scanAction.step,
              scanId: scanAction.scanId,
            }),
          ),
      });
    }

    items.push({
      id: "new-scan",
      label: "New Scan",
      onClick: () =>
        navigate(buildScanWizardUrl(target.projectId, target.id, { step: 2 })),
    });

    if (project) {
      items.push({
        id: "view-project",
        label: "View Project",
        onClick: () => navigate(`/projects/${project.id}`),
      });
    }

    items.push({
      id: "delete",
      label: "Delete Target",
      tone: "danger",
      disabled: deleting,
      onClick: () => void handleDeleteTarget(),
    });

    return items;
  }, [deleting, handleDeleteTarget, navigate, project, scanAction, target]);

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

  const primaryScanCta = scanAction
    ? primaryScanAction(scanAction, target, navigate, scanContext.scanStatusLabel)
    : null;

  return (
    <div className="page target-details">
      <PageHeader
        backTo="/targets"
        backOnly
        title={target.name}
        actions={
          <div className="page-actions">
            {primaryScanCta ? (
              <Button variant="primary" onClick={primaryScanCta.onClick}>
                {primaryScanCta.label}
              </Button>
            ) : null}
            <ActionsDropdown
              label="Target actions"
              disabled={deleting}
              items={actionItems}
            />
          </div>
        }
      />

      <p className="target-details__lead mono">{target.url}</p>

      <section className="target-details__overview" aria-label="Target overview">
        <Card className="detail-section target-details__meta">
          <h2 className="detail-section__title">Target information</h2>
          <div className="detail-section__body">
            <DetailRow label="Name" value={target.name} />
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
            <DetailRow label="Type" value={targetDisplayType(target)} />
            <DetailRow label="Authentication" value={target.authType} capitalize />
            <DetailRow label="Status" value={<StatusBadge status={target.status} />} />
          </div>
        </Card>

        <Card className="detail-section target-details__scan-panel">
          <h2 className="detail-section__title">Scan status</h2>
          <div className="target-details__scan-status">
            <TargetScanStatusBadge label={scanContext.scanStatusLabel} />
          </div>
          <div className="detail-summary-grid detail-summary-grid--metrics target-details__scan-metrics">
            <div className="summary-stat">
              <span className="summary-stat__label">Attack scans</span>
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

      <section className="target-details__primary" aria-label="Attack scans">
        <Card className="detail-section">
          <div className="detail-section__header">
            <div>
              <h2 className="detail-section__title">Attack scans</h2>
              <p className="detail-section__hint">
                {attackScans.length === 0
                  ? "Run a scan to test this target."
                  : `Showing ${Math.min(recentAttackScans.length, 5)} of ${attackScans.length} scans`}
              </p>
            </div>
            <div className="detail-section__header-actions">
              {attackScans.length > 0 ? (
                <Link to="/scans" className="link">
                  View all
                </Link>
              ) : null}
              <Button
                variant="primary"
                size="sm"
                onClick={() =>
                  navigate(buildScanWizardUrl(target.projectId, target.id, { step: 2 }))
                }
              >
                New scan
              </Button>
            </div>
          </div>

          {recentAttackScans.length === 0 ? (
            <EmptyState
              title="No attack scans yet"
              description="Start a scan to run security tests against this target."
              action={
                <Button
                  variant="primary"
                  size="sm"
                  onClick={() =>
                    navigate(buildScanWizardUrl(target.projectId, target.id, { step: 2 }))
                  }
                >
                  Start scan
                </Button>
              }
            />
          ) : (
            <ul className="detail-list">
              {recentAttackScans.map((scan) => (
                <li key={scan.id} className="detail-list-row detail-list-row--scans">
                  <button
                    type="button"
                    className="detail-list-link detail-list-row__title"
                    onClick={() => navigate(`/scans/${scan.id}`)}
                  >
                    {scan.name}
                  </button>
                  <StatusBadge status={scan.status} />
                  <span className="text-muted text-sm detail-list-row__meta">
                    {formatTimestamp(scan.startedAt ?? scan.createdAt)}
                  </span>
                </li>
              ))}
            </ul>
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

function primaryScanAction(
  action: TargetScanAction,
  target: { id: string; projectId: string },
  navigate: (path: string) => void,
  scanStatusLabel: string,
): { label: string; onClick: () => void } {
  if (action.kind === "view_scan") {
    return {
      label: "View Scan Progress",
      onClick: () =>
        navigate(buildScanProgressUrl(target.projectId, action.scanId, target.id)),
    };
  }
  if (action.kind === "view_report") {
    return {
      label: "View Scan Details",
      onClick: () => navigate(`/scans/${action.scanId}`),
    };
  }
  if (action.kind === "retry") {
    return {
      label: "Retry scan",
      onClick: () =>
        navigate(
          buildScanWizardUrl(target.projectId, target.id, {
            step: action.step,
            scanId: action.scanId,
          }),
        ),
    };
  }
  if (action.kind === "setup" && scanStatusLabel === "Never Scanned") {
    return {
      label: "New scan",
      onClick: () =>
        navigate(buildScanWizardUrl(target.projectId, target.id, { step: 2 })),
    };
  }
  if (action.kind === "setup") {
    return {
      label: "Continue setup",
      onClick: () =>
        navigate(
          buildScanWizardUrl(target.projectId, target.id, {
            step: action.step,
            scanId: action.scanId,
          }),
        ),
    };
  }

  return {
    label: "New scan",
    onClick: () =>
      navigate(buildScanWizardUrl(target.projectId, target.id, { step: 2 })),
  };
}
