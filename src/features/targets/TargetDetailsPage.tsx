import { useCallback, useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  ActionsDropdown,
  type ActionsDropdownItem,
  Card,
  EmptyState,
  PageHeader,
  StatusBadge,
  TargetScanStatusBadge,
} from "@/shared/components";
import { formatTimestamp } from "@/features/scans/scanDetailsHelpers";
import { targetDisplayType } from "@/features/scans/targetProfile";
import {
  buildScanWizardUrl,
  peekWizardSession,
  wizardResumeInputFromSession,
} from "@/features/scans/wizardState";
import {
  buildTargetScanContext,
  formatTargetTimestamp,
} from "@/shared/targetScanContext";
import { resolveTargetScanAction } from "@/shared/targetScanAction";
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

  const recentAttackScans = useMemo(
    () => targetScans.filter(isAttackScan).slice(0, 5),
    [targetScans],
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
        label: "View Scan",
        onClick: () =>
          navigate(
            buildScanWizardUrl(target.projectId, target.id, {
              scanId: scanAction.scanId,
              step: 5,
            }),
          ),
      });
    } else if (scanAction.kind === "view_report") {
      items.push({
        id: "view-scan",
        label: "View Scan",
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
      <div className="page">
        <PageHeader title="Target Details" backTo="/targets" backOnly description="Loading target…" />
      </div>
    );
  }

  return (
    <div className="page">
      <PageHeader
        backTo="/targets"
        backOnly
        title={target.name}
        actions={
          <ActionsDropdown
            label="Target actions"
            disabled={deleting}
            items={actionItems}
          />
        }
      />

      <div className="detail-sections">
        <Card className="detail-section">
          <h2 className="detail-section__title">Target Information</h2>
          <div className="detail-section__body">
            <DetailRow label="Name" value={target.name} />
            <DetailRow label="URL" value={target.url} mono />
            <DetailRow label="Project" value={project?.name ?? "—"} />
            <DetailRow label="Type" value={targetDisplayType(target)} />
            <DetailRow label="Authentication" value={target.authType} />
            <DetailRow label="Status" value={<StatusBadge status={target.status} />} />
          </div>
        </Card>

        <Card className="detail-section">
          <h2 className="detail-section__title">Scan Context</h2>
          <div className="detail-section__body">
            <DetailRow
              label="Scan Status"
              value={<TargetScanStatusBadge label={scanContext.scanStatusLabel} />}
            />
            <DetailRow label="Last Scan" value={formatTargetTimestamp(scanContext.lastScanTime)} />
            <DetailRow label="Latest Result" value={scanContext.latestScanResult} />
          </div>
        </Card>

        <Card className="detail-section">
          <h2 className="detail-section__title">Recent Attack Scans</h2>
          {recentAttackScans.length === 0 ? (
            <p className="text-muted">No attack scans recorded for this target.</p>
          ) : (
            <ul className="detail-list">
              {recentAttackScans.map((scan) => (
                <li key={scan.id} className="detail-list-row">
                  <button
                    type="button"
                    className="detail-list-link"
                    onClick={() => navigate(`/scans/${scan.id}`)}
                  >
                    {scan.name}
                  </button>
                  <StatusBadge status={scan.status} />
                  <span className="text-muted text-sm">
                    {formatTimestamp(scan.startedAt ?? scan.createdAt)}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </Card>
      </div>
    </div>
  );
}

function DetailRow({
  label,
  value,
  mono = false,
}: {
  label: string;
  value: React.ReactNode;
  mono?: boolean;
}) {
  return (
    <div className="detail-row">
      <span className="detail-row__label">{label}</span>
      <span className={`detail-row__value ${mono ? "mono" : ""}`}>{value}</span>
    </div>
  );
}
