import { useCallback, useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { AiRuntimeDashboardCard } from "@/features/dashboard/AiRuntimeDashboardCard";
import { severityCounts } from "@/shared/stats";
import { getRuntimeConfiguration, type RuntimeConfigurationDto } from "@/shared/ipc/runtime";
import { assertAiRuntimeReady } from "@/shared/runtime/aiRuntimeReadiness";
import {
  Badge,
  Button,
  Card,
  PageHeader,
  ProgressBar,
  SeverityBadge,
  StatCard,
} from "@/shared/components";
import { useToast } from "@/shared/notifications";

function formatRelativeTime(iso: string) {
  const diff = Date.now() - new Date(iso).getTime();
  const hours = Math.floor(diff / 3_600_000);
  if (hours < 1) return "Just now";
  if (hours < 24) return `${hours}h ago`;
  return `${Math.floor(hours / 24)}d ago`;
}

export function DashboardPage() {
  const { stats, findings, activity, discoveryJobs, attackRuns, projects, backendConnected } =
    useAppStore();
  const navigate = useNavigate();
  const { notify } = useToast();
  const counts = severityCounts(findings);
  const maxCount = Math.max(...Object.values(counts), 1);
  const [runtimeConfiguration, setRuntimeConfiguration] = useState<RuntimeConfigurationDto | null>(
    null,
  );
  const [runtimeLoading, setRuntimeLoading] = useState(false);
  const [openingProject, setOpeningProject] = useState(false);

  const loadRuntimeConfiguration = useCallback(async () => {
    if (!backendConnected) {
      setRuntimeConfiguration(null);
      return;
    }
    setRuntimeLoading(true);
    try {
      setRuntimeConfiguration(await getRuntimeConfiguration());
    } catch {
      setRuntimeConfiguration(null);
    } finally {
      setRuntimeLoading(false);
    }
  }, [backendConnected]);

  useEffect(() => {
    void loadRuntimeConfiguration();
  }, [loadRuntimeConfiguration]);

  async function handleNewProject() {
    if (openingProject) return;
    setOpeningProject(true);
    try {
      const readiness = await assertAiRuntimeReady(backendConnected);
      if (!readiness.ready) {
        notify(readiness.message, "error");
        navigate("/runtime");
        return;
      }
      navigate("/projects", { state: { openNewProject: true } });
    } finally {
      setOpeningProject(false);
    }
  }

  return (
    <div className="page dashboard-page">
      <PageHeader
        title="Dashboard"
        description="Overview of your AI security testing workspace"
        actions={
          <Button
            variant="primary"
            disabled={openingProject}
            onClick={() => void handleNewProject()}
          >
            {openingProject ? "Checking AI Runtime…" : "New Project"}
          </Button>
        }
      />

      <div className="stat-grid">
        <StatCard
          label="Projects"
          value={stats.projects}
          hint={`${stats.activeProjects} active`}
        />
        <StatCard
          label="Targets"
          value={stats.targets}
          hint={`${stats.scanningTargets} scanning`}
          accent="warning"
        />
        <StatCard
          label="Open Findings"
          value={stats.openFindings}
          hint={`${stats.criticalFindings} critical`}
          accent="critical"
        />
        <AiRuntimeDashboardCard configuration={runtimeConfiguration} loading={runtimeLoading} />
      </div>

      <div className="dashboard-grid">
        <Card className="dashboard-grid__chart">
          <h3 className="card__title">Findings by Severity</h3>
          <div className="severity-chart">
            {(["critical", "high", "medium", "low", "info"] as const).map((sev) => (
              <div key={sev} className="severity-chart__row">
                <SeverityBadge severity={sev} />
                <div className="severity-chart__bar-track">
                  <div
                    className={`severity-chart__bar severity-chart__bar--${sev}`}
                    style={{ width: `${(counts[sev] / maxCount) * 100}%` }}
                  />
                </div>
                <span className="severity-chart__count">{counts[sev]}</span>
              </div>
            ))}
          </div>
        </Card>

        <Card className="dashboard-grid__activity">
          <h3 className="card__title">Recent Activity</h3>
          <ul className="activity-list">
            {activity.map((item) => (
              <li key={item.id} className="activity-list__item">
                <span className={`activity-list__dot activity-list__dot--${item.type}`} />
                <div className="activity-list__body">
                  <p className="activity-list__message">{item.message}</p>
                  <time className="activity-list__time">{formatRelativeTime(item.timestamp)}</time>
                </div>
                {item.severity && <SeverityBadge severity={item.severity} />}
              </li>
            ))}
          </ul>
        </Card>

        <Card className="dashboard-grid__jobs">
          <div className="card__header-row">
            <h3 className="card__title">Active Jobs</h3>
            <Link to="/discovery" className="link">View all</Link>
          </div>
          {discoveryJobs
            .filter((j) => j.status === "running")
            .map((job) => (
              <ProgressBar
                key={job.id}
                label={`Discovery: ${job.targetName}`}
                value={job.progress}
              />
            ))}
          {attackRuns
            .filter((a) => a.status === "running")
            .map((a) => (
              <ProgressBar
                key={a.id}
                label={`${a.category.replace(/_/g, " ")} — ${a.targetName}`}
                value={a.payloadsRun}
                max={a.payloadsTotal}
              />
            ))}
          {stats.runningScans === 0 &&
            attackRuns.every((a) => a.status !== "running") && (
            <p className="text-muted">No active scans. Start discovery on a target.</p>
          )}
        </Card>

        <Card className="dashboard-grid__projects">
          <div className="card__header-row">
            <h3 className="card__title">Projects</h3>
            <Link to="/projects" className="link">Manage</Link>
          </div>
          <ul className="project-list-compact">
            {projects.slice(0, 3).map((p) => (
              <li key={p.id} className="project-list-compact__item">
                <div>
                  <strong>{p.name}</strong>
                  <span className="text-muted">{p.targetCount} targets · {p.findingCount} findings</span>
                </div>
                <Badge variant={p.status === "active" ? "success" : "muted"}>{p.status}</Badge>
              </li>
            ))}
          </ul>
        </Card>
      </div>
    </div>
  );
}
