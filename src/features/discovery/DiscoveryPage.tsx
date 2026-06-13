import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  Button,
  Card,
  EmptyState,
  PageHeader,
  StatusBadge,
} from "@/shared/components";
import { useToast } from "@/shared/notifications";
import type { DiscoveryStatsDto } from "@/shared/ipc";
import type { Project, ScanRun, Target } from "@/shared/types";

import { formatDurationMs, formatTimestamp } from "@/features/scans/scanDetailsHelpers";

type TargetDiscoveryGroup = {
  target: Target;
  runs: ScanRun[];
};

type ProjectDiscoveryGroup = {
  project: Project;
  targets: TargetDiscoveryGroup[];
};

function buildDiscoveryTree(
  projects: Project[],
  targets: Target[],
  scans: ScanRun[],
): ProjectDiscoveryGroup[] {
  const discoveryRuns = scans
    .filter((scan) => scan.name.startsWith("Discovery:"))
    .sort((a, b) => b.createdAt.localeCompare(a.createdAt));

  return projects
    .map((project) => {
      const projectTargets = targets.filter((target) => target.projectId === project.id);
      const targetGroups = projectTargets
        .map((target) => ({
          target,
          runs: discoveryRuns.filter((run) => run.targetId === target.id),
        }))
        .filter((group) => group.runs.length > 0);

      return { project, targets: targetGroups };
    })
    .filter((group) => group.targets.length > 0);
}

function scanDuration(run: ScanRun): string {
  if (run.startedAt && run.completedAt) {
    return formatDurationMs(
      new Date(run.completedAt).getTime() - new Date(run.startedAt).getTime(),
    );
  }
  return "—";
}

export function DiscoveryPage() {
  const { targets, scans, endpoints, projects, loading, error, actions } = useAppStore();
  const { notify } = useToast();
  const [targetId, setTargetId] = useState("");
  const [running, setRunning] = useState(false);
  const [lastStats, setLastStats] = useState<DiscoveryStatsDto | null>(null);
  const [collapsedProjects, setCollapsedProjects] = useState<Set<string>>(new Set());
  const [collapsedTargets, setCollapsedTargets] = useState<Set<string>>(new Set());

  useEffect(() => {
    if (!targetId && targets.length > 0) {
      setTargetId(targets[0].id);
    }
  }, [targets, targetId]);

  const tree = useMemo(
    () => buildDiscoveryTree(projects, targets, scans),
    [projects, targets, scans],
  );

  const endpointsByScan = useMemo(() => {
    const map = new Map<string, number>();
    for (const endpoint of endpoints) {
      map.set(endpoint.scanId, (map.get(endpoint.scanId) ?? 0) + 1);
    }
    return map;
  }, [endpoints]);

  const aiEndpointsByScan = useMemo(() => {
    const map = new Map<string, number>();
    for (const endpoint of endpoints) {
      if (endpoint.kind !== "ai_endpoint") continue;
      map.set(endpoint.scanId, (map.get(endpoint.scanId) ?? 0) + 1);
    }
    return map;
  }, [endpoints]);

  const selectedTarget = targets.find((target) => target.id === targetId) ?? null;

  async function handleStartScan() {
    if (!targetId || running) return;
    setRunning(true);
    setLastStats(null);
    try {
      const result = await actions.runDiscovery(targetId);
      setLastStats(result.stats);
      notify(
        `Discovery complete — ${result.stats.endpoint_count} endpoint(s) found`,
        "success",
      );
    } catch (err) {
      const message = err instanceof Error ? err.message : "Discovery failed";
      notify(message, "error");
    } finally {
      setRunning(false);
    }
  }

  function toggleProject(projectId: string) {
    setCollapsedProjects((prev) => {
      const next = new Set(prev);
      if (next.has(projectId)) next.delete(projectId);
      else next.add(projectId);
      return next;
    });
  }

  function toggleTarget(targetIdToToggle: string) {
    setCollapsedTargets((prev) => {
      const next = new Set(prev);
      if (next.has(targetIdToToggle)) next.delete(targetIdToToggle);
      else next.add(targetIdToToggle);
      return next;
    });
  }

  const noTargets = targets.length === 0;

  return (
    <div className="page">
      <PageHeader
        title="Discovery"
        description="Attack surface enumeration — crawl, API, OpenAPI, AI fingerprinting"
        actions={
          <div className="discovery-controls">
            <select
              className="input"
              value={targetId}
              onChange={(e) => setTargetId(e.target.value)}
              disabled={noTargets || running}
            >
              {noTargets && <option value="">No targets — add one first</option>}
              {targets.map((target) => (
                <option key={target.id} value={target.id}>
                  {target.name} ({target.url || "no url"})
                </option>
              ))}
            </select>
            <Button
              variant="primary"
              onClick={handleStartScan}
              disabled={noTargets || running || !targetId}
            >
              {running ? "Scanning…" : "Start Scan"}
            </Button>
          </div>
        }
      />

      {error && (
        <Card>
          <p className="text-danger">Failed to load discovery data: {error}</p>
        </Card>
      )}

      {running && (
        <Card className="discovery-progress">
          <div className="discovery-progress__row">
            <div className="page-loader__spinner" />
            <div>
              <strong>Scanning {selectedTarget?.name ?? "target"}…</strong>
              <p className="text-muted text-sm">
                Crawling {selectedTarget?.url} and probing for APIs, OpenAPI, GraphQL and AI endpoints.
              </p>
            </div>
          </div>
        </Card>
      )}

      {lastStats && !running && (
        <Card>
          <h3 className="card__title">Last run</h3>
          <div className="discovery-card__stats">
            <div>
              <span className="discovery-card__stat-value">{lastStats.endpoint_count}</span>
              <span className="discovery-card__stat-label">Endpoints</span>
            </div>
            <div>
              <span className="discovery-card__stat-value">{lastStats.pages_fetched}</span>
              <span className="discovery-card__stat-label">Pages</span>
            </div>
            <div>
              <span className="discovery-card__stat-value">{lastStats.probes_sent}</span>
              <span className="discovery-card__stat-label">Probes</span>
            </div>
            <div>
              <span className="discovery-card__stat-value">{lastStats.duration_ms} ms</span>
              <span className="discovery-card__stat-label">Duration</span>
            </div>
          </div>
        </Card>
      )}

      {tree.length === 0 && !running ? (
        <EmptyState
          title={loading ? "Loading…" : "No discovery runs yet"}
          description={
            noTargets
              ? "Add a target on the Targets page, then start a scan."
              : "Select a target and click Start Scan to enumerate its attack surface."
          }
        />
      ) : (
        <div className="discovery-tree">
          {tree.map((projectGroup) => {
            const projectExpanded = !collapsedProjects.has(projectGroup.project.id);
            return (
              <Card key={projectGroup.project.id} className="discovery-tree__project">
                <button
                  type="button"
                  className="discovery-tree__header"
                  onClick={() => toggleProject(projectGroup.project.id)}
                >
                  <strong>{projectGroup.project.name}</strong>
                  <span className="text-muted text-sm">
                    {projectGroup.targets.length} target
                    {projectGroup.targets.length === 1 ? "" : "s"}
                  </span>
                </button>

                {projectExpanded &&
                  projectGroup.targets.map((targetGroup) => {
                    const targetExpanded = !collapsedTargets.has(targetGroup.target.id);
                    return (
                      <div key={targetGroup.target.id} className="discovery-tree__target">
                        <button
                          type="button"
                          className="discovery-tree__target-header"
                          onClick={() => toggleTarget(targetGroup.target.id)}
                        >
                          <span className="mono text-sm">{targetGroup.target.url}</span>
                          <span className="text-muted text-sm">
                            {targetGroup.runs.length} run
                            {targetGroup.runs.length === 1 ? "" : "s"}
                          </span>
                        </button>

                        {targetExpanded && (
                          <ul className="discovery-tree__runs">
                            {targetGroup.runs.map((run, index) => (
                              <li key={run.id}>
                                <Link to={`/discovery/${run.id}`} className="discovery-run-link">
                                  <div>
                                    <strong>Discovery #{targetGroup.runs.length - index}</strong>
                                    <p className="text-muted text-sm">
                                      {formatTimestamp(run.completedAt ?? run.createdAt)}
                                    </p>
                                  </div>
                                  <div className="discovery-run-link__meta">
                                    <StatusBadge status={run.status} />
                                    <span>{endpointsByScan.get(run.id) ?? 0} endpoints</span>
                                    <span>{aiEndpointsByScan.get(run.id) ?? 0} AI</span>
                                    <span>{scanDuration(run)}</span>
                                  </div>
                                </Link>
                              </li>
                            ))}
                          </ul>
                        )}
                      </div>
                    );
                  })}
              </Card>
            );
          })}
        </div>
      )}
    </div>
  );
}
