import { useMemo, useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import {
  Badge,
  Button,
  Card,
  PageHeader,
  StatusBadge,
} from "@/shared/components";
import type { Project, ScanRun, Target } from "@/shared/types";

import { AddTargetModal } from "./AddTargetModal";

type ProjectTargetGroup = {
  project: Project;
  targets: Target[];
  targetCount: number;
  scannedCount: number;
  unscannedCount: number;
  runningCount: number;
};

function buildProjectGroups(
  projects: Project[],
  targets: Target[],
  scans: ScanRun[],
  selectedProjectId: string | null,
  query: string,
): ProjectTargetGroup[] {
  const normalizedQuery = query.toLowerCase().trim();

  return projects
    .filter((project) => !selectedProjectId || project.id === selectedProjectId)
    .map((project) => {
      const projectTargets = targets.filter((target) => {
        if (target.projectId !== project.id) return false;
        if (!normalizedQuery) return true;
        return (
          target.name.toLowerCase().includes(normalizedQuery) ||
          target.url.toLowerCase().includes(normalizedQuery)
        );
      });

      let scannedCount = 0;
      let runningCount = 0;

      for (const target of projectTargets) {
        const targetScans = scans.filter((scan) => scan.targetId === target.id);
        const hasCompleted = targetScans.some((scan) => scan.status === "completed");
        const hasRunning = targetScans.some(
          (scan) => scan.status === "running" || scan.status === "paused" || scan.status === "pending",
        );
        if (hasCompleted) scannedCount += 1;
        if (hasRunning) runningCount += 1;
      }

      return {
        project,
        targets: projectTargets,
        targetCount: projectTargets.length,
        scannedCount,
        unscannedCount: projectTargets.length - scannedCount,
        runningCount,
      };
    })
    .filter((group) => group.targetCount > 0);
}

export function TargetsPage() {
  const { targets, projects, scans, ui, loading, error, actions } = useAppStore();
  const [modalOpen, setModalOpen] = useState(false);
  const [collapsedProjects, setCollapsedProjects] = useState<Set<string>>(new Set());

  const groups = useMemo(
    () =>
      buildProjectGroups(
        projects,
        targets,
        scans,
        ui.selectedProjectId,
        ui.searchQuery,
      ),
    [projects, targets, scans, ui.selectedProjectId, ui.searchQuery],
  );

  function toggleProject(projectId: string) {
    setCollapsedProjects((prev) => {
      const next = new Set(prev);
      if (next.has(projectId)) next.delete(projectId);
      else next.add(projectId);
      return next;
    });
  }

  return (
    <div className="page">
      <PageHeader
        title="Targets"
        description="Endpoints, applications, and models under test"
        actions={
          <>
            <Button variant="ghost" onClick={() => void actions.refresh()} disabled={loading}>
              {loading ? "Refreshing…" : "Refresh"}
            </Button>
            <Button variant="primary" onClick={() => setModalOpen(true)}>
              Add Target
            </Button>
          </>
        }
      />

      {error && (
        <Card>
          <p className="text-danger">Failed to load targets: {error}</p>
        </Card>
      )}

      {groups.length === 0 ? (
        <Card>
          <p className="text-muted">
            {loading ? "Loading targets…" : "No targets yet. Add your first target."}
          </p>
        </Card>
      ) : (
        <div className="target-groups">
          {groups.map((group) => {
            const expanded = !collapsedProjects.has(group.project.id);
            return (
              <Card key={group.project.id} className="target-group">
                <button
                  type="button"
                  className="target-group__header"
                  onClick={() => toggleProject(group.project.id)}
                  aria-expanded={expanded}
                >
                  <div>
                    <h3 className="target-group__title">{group.project.name}</h3>
                    <div className="target-group__stats text-muted text-sm">
                      <span>{group.targetCount} targets</span>
                      <span>{group.scannedCount} scanned</span>
                      <span>{group.unscannedCount} unscanned</span>
                      <span>{group.runningCount} running</span>
                    </div>
                  </div>
                  <span className="target-group__toggle">{expanded ? "−" : "+"}</span>
                </button>

                {expanded && (
                  <ul className="target-group__list">
                    {group.targets.map((target) => (
                      <li key={target.id} className="target-group__item">
                        <div>
                          <strong>{target.name}</strong>
                          <div className="mono text-sm text-muted">{target.url}</div>
                        </div>
                        <div className="target-group__meta">
                          <Badge variant="info">{target.type}</Badge>
                          <StatusBadge status={target.status} />
                        </div>
                      </li>
                    ))}
                  </ul>
                )}
              </Card>
            );
          })}
        </div>
      )}

      <AddTargetModal open={modalOpen} onClose={() => setModalOpen(false)} />
    </div>
  );
}
