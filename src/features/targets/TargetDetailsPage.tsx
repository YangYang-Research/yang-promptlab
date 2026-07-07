import { useMemo } from "react";
import { useNavigate, useParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  Button,
  Card,
  EmptyState,
  PageHeader,
  StatusBadge,
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
import type { ScanRun } from "@/shared/types";

function isAttackScan(scan: ScanRun): boolean {
  return scan.name.startsWith("Scan (");
}

export function TargetDetailsPage() {
  const { targetId = "" } = useParams();
  const navigate = useNavigate();
  const { targets, projects, scans, loading } = useAppStore();

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
          <div className="page-actions">
            <Button
              variant="primary"
              onClick={() =>
                navigate(
                  buildScanWizardUrl(target.projectId, target.id, { step: 2 }),
                )
              }
            >
              New Scan
            </Button>
            {scanAction?.kind === "retry" && (
              <Button
                variant="secondary"
                onClick={() =>
                  navigate(
                    buildScanWizardUrl(target.projectId, target.id, {
                      step: scanAction.step,
                      scanId: scanAction.scanId,
                    }),
                  )
                }
              >
                Retry Scan
              </Button>
            )}
            {project && (
              <Button variant="secondary" onClick={() => navigate(`/projects/${project.id}`)}>
                View Project
              </Button>
            )}
          </div>
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
            <DetailRow label="Status" value={target.status} />
          </div>
        </Card>

        <Card className="detail-section">
          <h2 className="detail-section__title">Scan Context</h2>
          <div className="detail-section__body">
            <DetailRow label="Scan Status" value={scanContext.scanStatusLabel} />
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
  value: string;
  mono?: boolean;
}) {
  return (
    <div className="detail-row">
      <span className="detail-row__label">{label}</span>
      <span className={`detail-row__value ${mono ? "mono" : ""}`}>{value}</span>
    </div>
  );
}
