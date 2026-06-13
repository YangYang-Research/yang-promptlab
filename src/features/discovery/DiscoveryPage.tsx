import { useMemo, useState } from "react";
import { Link, useNavigate } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  Card,
  ContentToolbar,
  DataTable,
  EmptyState,
  IconButton,
  IconDiscovery,
  PageHeader,
  Pagination,
  RefreshButton,
  StatusBadge,
} from "@/shared/components";
import { usePageSizePreference } from "@/shared/hooks/usePageSizePreference";
import { usePaginatedList } from "@/shared/hooks/usePaginatedList";
import { useViewPreference } from "@/shared/hooks/useViewPreference";
import { useToast } from "@/shared/notifications";
import type { Project, ScanRun, Target } from "@/shared/types";

import { formatDurationMs, formatTimestamp } from "@/features/scans/scanDetailsHelpers";

type DiscoveryTableRow = {
  id: string;
  targetId: string;
  projectId: string;
  projectName: string;
  targetUrl: string;
  targetName: string;
  runLabel: string;
  run: ScanRun | null;
  endpointCount: number;
  aiEndpointCount: number;
  duration: string;
  date: string;
};

function buildDiscoveryRows(
  projects: Project[],
  targets: Target[],
  scans: ScanRun[],
  endpointsByScan: Map<string, number>,
  aiEndpointsByScan: Map<string, number>,
): DiscoveryTableRow[] {
  const discoveryRuns = scans
    .filter((scan) => scan.name.startsWith("Discovery:"))
    .sort((a, b) => b.createdAt.localeCompare(a.createdAt));

  const rows: DiscoveryTableRow[] = [];

  for (const project of projects) {
    const projectTargets = targets.filter((target) => target.projectId === project.id);
    for (const target of projectTargets) {
      const targetRuns = discoveryRuns.filter((run) => run.targetId === target.id);
      if (targetRuns.length === 0) {
        rows.push({
          id: `target-${target.id}`,
          targetId: target.id,
          projectId: project.id,
          projectName: project.name,
          targetUrl: target.url,
          targetName: target.name,
          runLabel: "No runs yet",
          run: null,
          endpointCount: 0,
          aiEndpointCount: 0,
          duration: "—",
          date: "—",
        });
        continue;
      }

      targetRuns.forEach((run, index) => {
        rows.push({
          id: run.id,
          targetId: target.id,
          projectId: project.id,
          projectName: project.name,
          targetUrl: target.url,
          targetName: target.name,
          runLabel: `Discovery #${targetRuns.length - index}`,
          run,
          endpointCount: endpointsByScan.get(run.id) ?? 0,
          aiEndpointCount: aiEndpointsByScan.get(run.id) ?? 0,
          duration: scanDuration(run),
          date: formatTimestamp(run.completedAt ?? run.createdAt),
        });
      });
    }
  }

  return rows.sort((a, b) => {
    const aTime = a.run?.createdAt ?? "";
    const bTime = b.run?.createdAt ?? "";
    return bTime.localeCompare(aTime);
  });
}

function scanDuration(run: ScanRun): string {
  if (run.startedAt && run.completedAt) {
    return formatDurationMs(
      new Date(run.completedAt).getTime() - new Date(run.startedAt).getTime(),
    );
  }
  return "—";
}

function buildDiscoveryTree(
  projects: Project[],
  targets: Target[],
  scans: ScanRun[],
) {
  const discoveryRuns = scans
    .filter((scan) => scan.name.startsWith("Discovery:"))
    .sort((a, b) => b.createdAt.localeCompare(a.createdAt));

  return projects
    .map((project) => {
      const projectTargets = targets.filter((target) => target.projectId === project.id);
      const targetGroups = projectTargets.map((target) => ({
        target,
        runs: discoveryRuns.filter((run) => run.targetId === target.id),
      }));

      return { project, targets: targetGroups.filter((group) => group.runs.length > 0) };
    })
    .filter((group) => group.targets.length > 0);
}

export function DiscoveryPage() {
  const { targets, scans, endpoints, projects, loading, error, actions } = useAppStore();
  const { notify } = useToast();
  const navigate = useNavigate();
  const [viewMode, setViewMode] = useViewPreference("discovery");
  const [pageSize, setPageSize] = usePageSizePreference("discovery");
  const [runningTargetId, setRunningTargetId] = useState<string | null>(null);
  const [collapsedProjects, setCollapsedProjects] = useState<Set<string>>(new Set());
  const [collapsedTargets, setCollapsedTargets] = useState<Set<string>>(new Set());

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

  const tableRows = useMemo(
    () => buildDiscoveryRows(projects, targets, scans, endpointsByScan, aiEndpointsByScan),
    [projects, targets, scans, endpointsByScan, aiEndpointsByScan],
  );

  const tree = useMemo(
    () => buildDiscoveryTree(projects, targets, scans),
    [projects, targets, scans],
  );

  const { page, setPage, pagination } = usePaginatedList(tableRows, pageSize);

  async function handleRunDiscovery(targetId: string) {
    if (runningTargetId) return;
    setRunningTargetId(targetId);
    try {
      const result = await actions.runDiscovery(targetId);
      notify(
        `Discovery complete — ${result.stats.endpoint_count} endpoint(s) found`,
        "success",
      );
    } catch (err) {
      const message = err instanceof Error ? err.message : "Discovery failed";
      notify(message, "error");
    } finally {
      setRunningTargetId(null);
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

  const tableColumns = [
    {
      key: "project",
      header: "Project",
      render: (row: DiscoveryTableRow) => row.projectName,
    },
    {
      key: "target",
      header: "Target",
      render: (row: DiscoveryTableRow) => <span className="mono text-sm">{row.targetUrl}</span>,
    },
    {
      key: "run",
      header: "Discovery Run",
      render: (row: DiscoveryTableRow) => row.runLabel,
    },
    {
      key: "status",
      header: "Status",
      width: "110px",
      render: (row: DiscoveryTableRow) =>
        row.run ? <StatusBadge status={row.run.status} /> : <span className="text-muted">Never run</span>,
    },
    {
      key: "endpoints",
      header: "Endpoints Found",
      width: "130px",
      render: (row: DiscoveryTableRow) => row.endpointCount,
    },
    {
      key: "ai",
      header: "AI Endpoints",
      width: "120px",
      render: (row: DiscoveryTableRow) => row.aiEndpointCount,
    },
    {
      key: "duration",
      header: "Duration",
      width: "100px",
      render: (row: DiscoveryTableRow) => row.duration,
    },
    {
      key: "date",
      header: "Date",
      width: "160px",
      render: (row: DiscoveryTableRow) => row.date,
    },
    {
      key: "actions",
      header: "",
      width: "72px",
      render: (row: DiscoveryTableRow) => (
        <span className="table-actions" onClick={(event) => event.stopPropagation()}>
          <IconButton
            ariaLabel="Run discovery"
            disabled={runningTargetId === row.targetId}
            onClick={() => void handleRunDiscovery(row.targetId)}
          >
            <IconDiscovery />
          </IconButton>
        </span>
      ),
    },
  ];

  const noTargets = targets.length === 0;

  return (
    <div className="page">
      <PageHeader
        title="Discovery"
        description="Attack surface enumeration — crawl, API, OpenAPI, AI fingerprinting"
        actions={
          <RefreshButton loading={loading} onClick={() => void actions.refresh()} />
        }
      />

      {error && (
        <Card>
          <p className="text-danger">Failed to load discovery data: {error}</p>
        </Card>
      )}

      {runningTargetId && (
        <Card className="discovery-progress">
          <div className="discovery-progress__row">
            <div className="page-loader__spinner" />
            <div>
              <strong>Running discovery…</strong>
              <p className="text-muted text-sm">Enumerating attack surface for the selected target.</p>
            </div>
          </div>
        </Card>
      )}

      {tableRows.length === 0 && !loading ? (
        <EmptyState
          title={loading ? "Loading…" : "No targets available"}
          description={
            noTargets
              ? "Add a target on the Targets page, then run discovery from the table."
              : "No discovery data available yet."
          }
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
            <Card padding="none">
              <DataTable
                columns={tableColumns}
                rows={pagination.items}
                keyField="id"
                onRowClick={(row) => row.run && navigate(`/discovery/${row.run.id}`)}
                emptyMessage={loading ? "Loading discovery runs…" : "No discovery runs found"}
              />
            </Card>
          ) : (
            <div className="discovery-tree">
              {tree.length === 0 ? (
                <Card>
                  <p className="text-muted">No discovery runs yet. Run discovery from table view.</p>
                </Card>
              ) : (
                tree.map((projectGroup) => {
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
                              <div className="discovery-tree__target-header-row">
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
                                <IconButton
                                  ariaLabel="Run discovery"
                                  disabled={runningTargetId === targetGroup.target.id}
                                  onClick={() => void handleRunDiscovery(targetGroup.target.id)}
                                >
                                  <IconDiscovery />
                                </IconButton>
                              </div>

                              {targetExpanded && (
                                <ul className="discovery-tree__runs">
                                  {targetGroup.runs.map((run, index) => (
                                    <li key={run.id}>
                                      <Link to={`/discovery/${run.id}`} className="discovery-run-link">
                                        <div>
                                          <strong>Discovery #{targetGroup.runs.length - index}</strong>
                                          <p className="card-footer-meta text-sm text-muted">
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
                })
              )}
            </div>
          )}

          {viewMode === "table" && (
            <Pagination
              page={page}
              totalItems={pagination.totalItems}
              rangeStart={pagination.rangeStart}
              rangeEnd={pagination.rangeEnd}
              totalPages={pagination.totalPages}
              onPageChange={setPage}
            />
          )}
        </>
      )}
    </div>
  );
}
