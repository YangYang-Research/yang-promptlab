import { useCallback, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  ActionsDropdown,
  type ActionsDropdownItem,
  Badge,
  Button,
  Card,
  ContentToolbar,
  DataTable,
  EmptyState,
  IconArrowRight,
  IconEdit,
  IconProgress,
  IconRefresh,
  IconTrash,
  ListCard,
  PageHeader,
  PageLoadingSkeleton,
  Pagination,
  SeverityBadge,
  StatusBadge,
} from "@/shared/components";
import {
  SeverityDoughnutChart,
  severitySliceColor,
} from "@/features/dashboard/SeverityDoughnutChart";
import { formatTimestamp } from "@/features/scans/scanDetailsHelpers";
import { NewScanChooserModal } from "@/features/scans/NewScanChooserModal";
import {
  buildScanProgressUrl,
  buildScanWizardUrl,
  peekWizardSession,
  wizardResumeInputFromSession,
} from "@/features/scans/wizardState";
import { targetDisplayType } from "@/features/scans/targetProfile";
import { severityCountSeries } from "@/shared/stats";
import { buildTargetScanContext, countAttackScans, formatTargetTimestamp } from "@/shared/targetScanContext";
import { resolveTargetScanAction } from "@/shared/targetScanAction";
import { usePageSizePreference } from "@/shared/hooks/usePageSizePreference";
import { usePaginatedList } from "@/shared/hooks/usePaginatedList";
import { useViewPreference } from "@/shared/hooks/useViewPreference";
import { useToast } from "@/shared/notifications";
import type { Severity, ScanRun, Target } from "@/shared/types";

import { EditProjectModal } from "./EditProjectModal";
import { ProjectSummaryPanel } from "./ProjectSummaryPanel";

const SEVERITIES: Severity[] = ["critical", "high", "medium", "low", "info"];

function formatDate(iso: string) {
  return new Date(iso).toLocaleString();
}

export function ProjectDetailsPage() {
  const { projectId = "" } = useParams();
  const navigate = useNavigate();
  const { projects, targets, scans, findings, loading, actions } = useAppStore();
  const { notify } = useToast();
  const [editOpen, setEditOpen] = useState(false);
  const [chooserOpen, setChooserOpen] = useState(false);
  const [deletingTargetId, setDeletingTargetId] = useState<string | null>(null);
  const [pageSize, setPageSize] = usePageSizePreference("project-details-targets");
  const [viewMode, setViewMode] = useViewPreference("project-details-targets");

  const project = projects.find((item) => item.id === projectId);

  const projectTargets = useMemo(
    () => targets.filter((target) => target.projectId === projectId),
    [targets, projectId],
  );

  const { page, setPage, pagination } = usePaginatedList(projectTargets, pageSize);

  const projectScans = useMemo(
    () => scans.filter((scan) => scan.projectId === projectId),
    [scans, projectId],
  );

  const projectFindings = useMemo(
    () => findings.filter((finding) => finding.projectId === projectId),
    [findings, projectId],
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
    let running = 0;
    let scannedTargets = 0;
    for (const target of projectTargets) {
      if (target.status === "scanned") scannedTargets += 1;
      const context = buildTargetScanContext(target.id, projectScans);
      if (context.scanStatusLabel === "Running") running += 1;
    }
    const targetIds = projectTargets.map((target) => target.id);
    return {
      targetCount: projectTargets.length,
      scanned: countAttackScans(targetIds, projectScans),
      unscanned: projectTargets.length - scannedTargets,
      running,
    };
  }, [projectTargets, projectScans]);

  const wizardSession = useMemo(() => peekWizardSession(), []);

  const recentFindings = useMemo(
    () =>
      [...projectFindings]
        .sort((a, b) => b.discoveredAt.localeCompare(a.discoveredAt))
        .slice(0, 5),
    [projectFindings],
  );

  const severitySlices = useMemo(
    () =>
      severityCountSeries(projectFindings).map((slice) => ({
        ...slice,
        color: severitySliceColor(slice.severity),
      })),
    [projectFindings],
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
        header: "Status",
        width: "120px",
        render: (target: Target) => <StatusBadge status={target.status} />,
      },
      {
        key: "scans",
        header: "Attack Scans",
        width: "110px",
        render: (target: Target) =>
          buildTargetScanContext(target.id, projectScans).scanCount,
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
              scans={scans}
              wizardSession={wizardSession}
              deleting={deletingTargetId === target.id}
              onNavigate={navigate}
              onDelete={handleDeleteTarget}
            />
          </span>
        ),
      },
    ],
    [projectId, projectScans, scans, navigate, wizardSession, deletingTargetId, handleDeleteTarget],
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
      <div className="page project-details">
        <PageHeader title="Project Details" backTo="/projects" backOnly />
        <PageLoadingSkeleton />
      </div>
    );
  }

  const openFindings = projectFindings.length;
  const openFindingsLabel =
    openFindings === 1 ? "1 finding" : `${openFindings} findings`;

  return (
    <div className="page project-details">
      <PageHeader
        backTo="/projects"
        backOnly
        title={project.name}
        actions={
          <div className="page-actions">
            <Button variant="primary" onClick={() => setChooserOpen(true)}>
              New Scan
            </Button>
            <ActionsDropdown
              items={[
                {
                  id: "edit",
                  label: "Edit Project",
                  icon: <IconEdit />,
                  onClick: () => setEditOpen(true),
                },
                {
                  id: "delete",
                  label: "Delete Project",
                  icon: <IconTrash />,
                  onClick: () => void handleDelete(),
                  tone: "danger",
                },
              ]}
            />
          </div>
        }
      />

      <NewScanChooserModal
        open={chooserOpen}
        onClose={() => setChooserOpen(false)}
        projectId={project.id}
      />

      <section className="project-details__overview" aria-label="Project overview">
        <Card className="detail-section project-details__meta">
          <h2 className="detail-section__title">Project Information</h2>
          <div className="detail-section__body">
            <DetailRow label="Name" value={project.name} />
            <DetailRow
              label="Description"
              value={project.description?.trim() ? project.description : "—"}
            />
            <DetailRow label="Created" value={formatDate(project.createdAt)} />
            <DetailRow label="Last updated" value={formatDate(project.updatedAt)} />
          </div>
        </Card>

        <Card className="detail-section project-details__target-stats">
          <h2 className="detail-section__title">Target Coverage</h2>
          <div className="detail-summary-grid detail-summary-grid--metrics">
            <SummaryStat label="Targets" value={targetSummary.targetCount} />
            <SummaryStat label="Scans" value={targetSummary.scanned} accent="success" />
            <SummaryStat label="Unscanned" value={targetSummary.unscanned} />
            <SummaryStat
              label="Running"
              value={targetSummary.running}
              accent={targetSummary.running > 0 ? "active" : undefined}
            />
          </div>
        </Card>
      </section>

      <section className="project-details__primary" aria-label="Targets">
        <Card className="detail-section project-details__targets-card">
          <div className="detail-section__header">
            <h2 className="detail-section__title">Targets</h2>
            <div className="detail-section__header-actions">
              <Button
                variant="primary"
                size="sm"
                onClick={() => navigate(buildScanWizardUrl(project.id, undefined, { step: 2 }))}
              >
                Add target
              </Button>
            </div>
          </div>

          {projectTargets.length === 0 ? (
            <EmptyState
              title="No targets yet"
              description="Add a target URL or API endpoint to include it in this project."
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
                <div className="project-details__targets-table">
                  <DataTable
                    columns={targetColumns}
                    rows={pagination.items}
                    keyField="id"
                    emptyMessage="No targets"
                    onRowClick={(target) => navigate(`/targets/${target.id}`)}
                  />
                </div>
              ) : (
                <div className="list-card-grid">
                  {pagination.items.map((target) => {
                    const scanContext = buildTargetScanContext(target.id, scans);
                    return (
                      <ListCard
                        key={target.id}
                        title={target.name}
                        status={<Badge variant="info">{targetDisplayType(target)}</Badge>}
                        metadata={[
                          {
                            label: "URL",
                            value: <span className="mono text-sm">{target.url}</span>,
                          },
                          { label: "Auth", value: target.authType },
                          { label: "Status", value: <StatusBadge status={target.status} /> },
                          {
                            label: "Scans",
                            value: `${scanContext.scanCount} scan${scanContext.scanCount === 1 ? "" : "s"}`,
                          },
                        ]}
                        footerMeta={
                          scanContext.lastScanTime
                            ? `Last scan: ${formatTargetTimestamp(scanContext.lastScanTime)}`
                            : "No scans recorded"
                        }
                        actions={
                          <span onClick={(event) => event.stopPropagation()}>
                            <TargetActionsDropdown
                              target={target}
                              projectId={projectId}
                              scans={scans}
                              wizardSession={wizardSession}
                              deleting={deletingTargetId === target.id}
                              onNavigate={navigate}
                              onDelete={handleDeleteTarget}
                            />
                          </span>
                        }
                        onClick={() => navigate(`/targets/${target.id}`)}
                      />
                    );
                  })}
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

      <section className="project-details__insights" aria-label="Findings and security overview">
        <Card className="detail-section project-details__findings-panel">
          <div className="detail-section__header">
            <div>
              <h2 className="detail-section__title">Findings</h2>
              <p className="detail-section__hint">{openFindingsLabel} in this project</p>
            </div>
            {openFindings > 0 ? (
              <Link to="/findings" className="link">
                View all
              </Link>
            ) : null}
          </div>

          <div className="detail-summary-grid detail-summary-grid--severity">
            {SEVERITIES.map((severity) => (
              <SummaryStat
                key={severity}
                severity={severity}
                value={severityCounts.get(severity) ?? 0}
              />
            ))}
          </div>

          <div className="project-details__subsection">
            <h3 className="project-details__subsection-title">Recent findings</h3>
            {recentFindings.length === 0 ? (
              <p className="text-muted text-sm">No findings recorded for this project yet.</p>
            ) : (
              <ul className="detail-list">
                {recentFindings.map((finding) => (
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
            )}
          </div>
        </Card>

        <Card className="detail-section project-details__security-overview">
          <h2 className="detail-section__title">Security Overview</h2>
          <SeverityDoughnutChart data={severitySlices} size={176} />
        </Card>
      </section>

      <section className="project-details__summary" aria-label="Project summary">
        <Card className="detail-section project-details__summary-panel scan-details__recommendations-card">
          <ProjectSummaryPanel
            projectId={projectId}
            enabled={projectTargets.length > 0}
          />
        </Card>
      </section>

      <EditProjectModal
        open={editOpen}
        project={project}
        onClose={() => setEditOpen(false)}
      />
    </div>
  );
}

function DetailRow({
  label,
  value,
  capitalize = false,
}: {
  label: string;
  value: string;
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

function SummaryStat({
  label,
  value,
  severity,
  accent,
}: {
  label?: string;
  value: number;
  severity?: Severity;
  accent?: "success" | "active";
}) {
  return (
    <div
      className={[
        "summary-stat",
        accent === "success" ? "summary-stat--success" : "",
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
        label: "View Scan Progress",
        icon: <IconProgress />,
        onClick: () =>
          onNavigate(buildScanProgressUrl(projectId, action.scanId, target.id)),
      });
    } else if (action.kind === "retry") {
      scanItems.push({
        id: "retry",
        label: "Retry Scan",
        icon: <IconRefresh />,
        onClick: () =>
          onNavigate(
            buildScanWizardUrl(projectId, target.id, {
              step: action.step,
              scanId: action.scanId,
            }),
          ),
      });
    } else if (action.kind !== "view_report") {
      scanItems.push({
        id: "setup",
        label: "Continue Setup",
        icon: <IconArrowRight />,
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
      icon: <IconTrash />,
      tone: "danger",
      disabled: deleting,
      onClick: () => onDelete(target),
    });

    return scanItems;
  }, [action, deleting, onDelete, onNavigate, projectId, target]);

  return <ActionsDropdown items={items} disabled={deleting} label="Target actions" />;
}
