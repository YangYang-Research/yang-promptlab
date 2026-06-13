import { useMemo, useState } from "react";
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
  SeverityBadge,
} from "@/shared/components";
import { formatTimestamp } from "@/features/scans/scanDetailsHelpers";
import { buildTargetScanContext } from "@/shared/targetScanContext";
import { useToast } from "@/shared/notifications";
import type { Severity, Target } from "@/shared/types";

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
        key: "auth",
        header: "Auth",
        width: "120px",
        render: (target: Target) => target.authType,
      },
      {
        key: "status",
        header: "Scan Status",
        width: "120px",
        render: (target: Target) => buildTargetScanContext(target.id, projectScans).scanStatusLabel,
      },
    ],
    [projectScans],
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
          <h2 className="detail-section__title">Recent Targets</h2>
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
                label={severity.charAt(0).toUpperCase() + severity.slice(1)}
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

function SummaryStat({ label, value }: { label: string; value: number }) {
  return (
    <div className="summary-stat">
      <span className="summary-stat__label">{label}</span>
      <span className="summary-stat__value">{value}</span>
    </div>
  );
}
