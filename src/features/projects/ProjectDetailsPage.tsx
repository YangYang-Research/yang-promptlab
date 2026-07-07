import { useCallback, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  ActionsDropdown,
  type ActionsDropdownItem,
  Badge,
  Button,
  Card,
  DataTable,
  EmptyState,
  PageHeader,
  SeverityBadge,
  TargetScanStatusBadge,
} from "@/shared/components";
import { formatTimestamp } from "@/features/scans/scanDetailsHelpers";
import {
  buildScanWizardUrl,
  peekWizardSession,
  wizardResumeInputFromSession,
} from "@/features/scans/wizardState";
import { targetDisplayType } from "@/features/scans/targetProfile";
import { buildTargetScanContext } from "@/shared/targetScanContext";
import { resolveTargetScanAction } from "@/shared/targetScanAction";
import { useToast } from "@/shared/notifications";
import type { Severity, ScanRun, Target } from "@/shared/types";

import { EditProjectModal } from "./EditProjectModal";

const SEVERITIES: Severity[] = ["critical", "high", "medium", "low", "info"];

function formatDate(iso: string) {
  return new Date(iso).toLocaleString();
}

export function ProjectDetailsPage() {
  const { projectId = "" } = useParams();
  const navigate = useNavigate();
  const { projects, targets, scans, findings, reports, loading, actions } = useAppStore();
  const { notify } = useToast();
  const [editOpen, setEditOpen] = useState(false);
  const [deletingTargetId, setDeletingTargetId] = useState<string | null>(null);

  const project = projects.find((item) => item.id === projectId);

  const projectTargets = useMemo(
    () => targets.filter((target) => target.projectId === projectId),
    [targets, projectId],
  );

  const projectScans = useMemo(
    () => scans.filter((scan) => scan.projectId === projectId),
    [scans, projectId],
  );

  const projectFindings = useMemo(
    () => findings.filter((finding) => finding.projectId === projectId),
    [findings, projectId],
  );

  const projectReports = useMemo(
    () => reports.filter((report) => report.projectId === projectId),
    [reports, projectId],
  );

  const severityCounts = useMemo(() => {
    const counts = new Map<Severity, number>();
    for (const severity of SEVERITIES) counts.set(severity, 0);
    for (const finding of projectFindings) {
      counts.set(finding.severity, (counts.get(finding.severity) ?? 0) + 1);
    }
    return counts;
  }, [projectFindings]);

  const targetSummary = useMemo(() => {
    let scanned = 0;
    let running = 0;
    for (const target of projectTargets) {
      const context = buildTargetScanContext(target.id, projectScans);
      if (context.scanStatusLabel === "Running") running += 1;
      if (context.scanStatusLabel === "Completed" || context.scanStatusLabel === "Failed") {
        scanned += 1;
      }
    }
    return {
      targetCount: projectTargets.length,
      scanned,
      unscanned: projectTargets.length - scanned,
      running,
    };
  }, [projectTargets, projectScans]);

  const wizardSession = useMemo(() => peekWizardSession(), []);

  const recentTargets = useMemo(
    () => [...projectTargets].slice(0, 5),
    [projectTargets],
  );

  const recentFindings = useMemo(
    () =>
      [...projectFindings]
        .sort((a, b) => b.discoveredAt.localeCompare(a.discoveredAt))
        .slice(0, 5),
    [projectFindings],
  );

  const recentReports = useMemo(
    () =>
      [...projectReports]
        .sort((a, b) => b.createdAt.localeCompare(a.createdAt))
        .slice(0, 5),
    [projectReports],
  );

  const handleDeleteTarget = useCallback(
    async (target: Target) => {
      const hasActiveScan = projectScans.some(
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

      setDeletingTargetId(target.id);
      try {
        await actions.deleteTarget(target.id);
        notify(`Target "${target.name}" deleted`, "success");
      } catch {
        notify("Failed to delete target", "error");
      } finally {
        setDeletingTargetId(null);
      }
    },
    [actions, notify, projectScans],
  );

  const targetColumns = useMemo(
    () => [
      {
        key: "url",
        header: "Target",
        render: (target: Target) => (
          <div>
            <strong>{target.name}</strong>
            <div className="mono text-sm text-muted">{target.url}</div>
          </div>
        ),
      },
      {
        key: "type",
        header: "Type",
        width: "80px",
        render: (target: Target) => <Badge variant="info">{targetDisplayType(target)}</Badge>,
      },
      {
        key: "auth",
        header: "Auth",
        width: "120px",
        render: (target: Target) => target.authType,
      },
      {
        key: "status",
        header: "Scan Status",
        width: "140px",
        render: (target: Target) => (
          <TargetScanStatusBadge
            label={buildTargetScanContext(target.id, projectScans).scanStatusLabel}
          />
        ),
      },
      {
        key: "actions",
        header: "",
        width: "56px",
        render: (target: Target) => (
          <span onClick={(event) => event.stopPropagation()}>
            <TargetActionsDropdown
              target={target}
              projectId={projectId}
              scans={projectScans}
              wizardSession={wizardSession}
              deleting={deletingTargetId === target.id}
              onNavigate={navigate}
              onDelete={handleDeleteTarget}
            />
          </span>
        ),
      },
    ],
    [projectScans, projectId, navigate, wizardSession, deletingTargetId, handleDeleteTarget],
  );

  async function handleDelete() {
    if (!project) return;
    try {
      await actions.deleteProject(project.id);
      notify(`Project "${project.name}" deleted`, "success");
      navigate("/projects");
    } catch {
      notify("Failed to delete project", "error");
    }
  }

  if (!project && !loading) {
    return (
      <div className="page">
        <PageHeader title="Project Details" backTo="/projects" backOnly />
        <EmptyState title="Project not found" description="This project may have been deleted." />
      </div>
    );
  }

  if (!project) {
    return (
      <div className="page">
        <PageHeader title="Project Details" backTo="/projects" backOnly description="Loading project…" />
      </div>
    );
  }

  return (
    <div className="page">
      <PageHeader
        backTo="/projects"
        backOnly
        title={project.name}
        actions={
          <div className="page-actions">
            <Link to={`/scans/new?projectId=${encodeURIComponent(project.id)}`}>
              <Button variant="primary">New Scan</Button>
            </Link>
            <ActionsDropdown
              items={[
                { id: "edit", label: "Edit Project", onClick: () => setEditOpen(true) },
                { id: "delete", label: "Delete Project", onClick: () => void handleDelete(), tone: "danger" },
              ]}
            />
          </div>
        }
      />

      <div className="detail-sections">
        <Card className="detail-section">
          <h2 className="detail-section__title">Project Information</h2>
          <div className="detail-section__body">
            <DetailRow label="Name" value={project.name} />
            <DetailRow label="Description" value={project.description || "—"} />
            <DetailRow label="Created" value={formatDate(project.createdAt)} />
            <DetailRow label="Last Updated" value={formatDate(project.updatedAt)} />
          </div>
        </Card>

        <Card className="detail-section">
          <h2 className="detail-section__title">Targets Summary</h2>
          <div className="detail-summary-grid">
            <SummaryStat label="Target Count" value={targetSummary.targetCount} />
            <SummaryStat label="Scanned Targets" value={targetSummary.scanned} />
            <SummaryStat label="Unscanned Targets" value={targetSummary.unscanned} />
            <SummaryStat label="Running Scans" value={targetSummary.running} />
          </div>
        </Card>

        <Card className="detail-section">
          <div className="card__header-row">
            <h2 className="detail-section__title card__title">Recent Targets</h2>
            <Button
              variant="primary"
              size="sm"
              onClick={() => navigate(buildScanWizardUrl(project.id, undefined, { step: 2 }))}
            >
              Add Target
            </Button>
          </div>
          {recentTargets.length === 0 ? (
            <p className="text-muted">No targets in this project yet.</p>
          ) : (
            <DataTable
              columns={targetColumns}
              rows={recentTargets}
              keyField="id"
              emptyMessage="No targets"
              onRowClick={(target) => navigate(`/targets/${target.id}`)}
            />
          )}
        </Card>

        <Card className="detail-section">
          <h2 className="detail-section__title">Findings Summary</h2>
          <div className="detail-summary-grid">
            {SEVERITIES.map((severity) => (
              <SummaryStat
                key={severity}
                severity={severity}
                value={severityCounts.get(severity) ?? 0}
              />
            ))}
          </div>
        </Card>

        <Card className="detail-section">
          <h2 className="detail-section__title">Recent Findings</h2>
          {recentFindings.length === 0 ? (
            <p className="text-muted">No findings recorded for this project.</p>
          ) : (
            <ul className="detail-list">
              {recentFindings.map((finding) => (
                <li key={finding.id} className="detail-list-row">
                  <SeverityBadge severity={finding.severity} />
                  <span>{finding.title}</span>
                  <span className="text-muted text-sm">{formatTimestamp(finding.discoveredAt)}</span>
                </li>
              ))}
            </ul>
          )}
        </Card>

        <Card className="detail-section">
          <h2 className="detail-section__title">Reports</h2>
          {recentReports.length === 0 ? (
            <p className="text-muted">No reports generated for this project.</p>
          ) : (
            <ul className="detail-list">
              {recentReports.map((report) => (
                <li key={report.id} className="detail-list-row">
                  <span>{report.title}</span>
                  <Badge variant="muted">{report.format.toUpperCase()}</Badge>
                  <span className="text-muted text-sm">{formatTimestamp(report.createdAt)}</span>
                </li>
              ))}
            </ul>
          )}
        </Card>
      </div>

      <EditProjectModal
        open={editOpen}
        project={project}
        onClose={() => setEditOpen(false)}
      />
    </div>
  );
}

function DetailRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="detail-row">
      <span className="detail-row__label">{label}</span>
      <span className="detail-row__value">{value}</span>
    </div>
  );
}

function SummaryStat({
  label,
  value,
  severity,
}: {
  label?: string;
  value: number;
  severity?: Severity;
}) {
  return (
    <div className="summary-stat">
      {severity ? (
        <SeverityBadge severity={severity} />
      ) : (
        <span className="summary-stat__label">{label}</span>
      )}
      <span className="summary-stat__value">{value}</span>
    </div>
  );
}

function TargetActionsDropdown({
  target,
  projectId,
  scans,
  wizardSession,
  deleting,
  onNavigate,
  onDelete,
}: {
  target: Target;
  projectId: string;
  scans: ScanRun[];
  wizardSession: ReturnType<typeof peekWizardSession>;
  deleting: boolean;
  onNavigate: (path: string) => void;
  onDelete: (target: Target) => void;
}) {
  const action = useMemo(
    () =>
      resolveTargetScanAction(
        target.id,
        projectId,
        scans,
        wizardSession ? wizardResumeInputFromSession(wizardSession) : null,
      ),
    [target.id, projectId, scans, wizardSession],
  );

  const items = useMemo(() => {
    const scanItems: ActionsDropdownItem[] = [];

    if (action.kind === "view_scan") {
      scanItems.push({
        id: "view-scan",
        label: "View Scan",
        onClick: () =>
          onNavigate(
            buildScanWizardUrl(projectId, target.id, {
              scanId: action.scanId,
              step: 5,
            }),
          ),
      });
    } else if (action.kind === "view_report") {
      scanItems.push({
        id: "view-report",
        label: "View Scan",
        onClick: () => onNavigate(`/scans/${action.scanId}`),
      });
    } else if (action.kind === "retry") {
      scanItems.push({
        id: "retry",
        label: "Retry Scan",
        onClick: () =>
          onNavigate(
            buildScanWizardUrl(projectId, target.id, {
              step: action.step,
              scanId: action.scanId,
            }),
          ),
      });
    } else {
      scanItems.push({
        id: "setup",
        label: "Continue Setup",
        onClick: () =>
          onNavigate(
            buildScanWizardUrl(projectId, target.id, {
              step: action.step,
              scanId: action.scanId,
            }),
          ),
      });
    }

    scanItems.push({
      id: "delete",
      label: "Delete Target",
      tone: "danger",
      disabled: deleting,
      onClick: () => onDelete(target),
    });

    return scanItems;
  }, [action, deleting, onDelete, onNavigate, projectId, target]);

  return <ActionsDropdown items={items} disabled={deleting} label="Target actions" />;
}
