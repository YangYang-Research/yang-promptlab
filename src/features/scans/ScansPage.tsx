import { useCallback, useMemo, useState } from "react";
import { Link, useNavigate } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  Button,
  Card,
  ContentToolbar,
  DataTable,
  EmptyState,
  PageHeader,
  Pagination,
  RefreshButton,
  StatusBadge,
} from "@/shared/components";
import { usePageSizePreference } from "@/shared/hooks/usePageSizePreference";
import { usePaginatedList } from "@/shared/hooks/usePaginatedList";
import { useViewPreference } from "@/shared/hooks/useViewPreference";
import { pauseScan, resumeScan, stopScan } from "@/shared/ipc";
import { useToast } from "@/shared/notifications";
import { formatDurationMs, formatTimestamp } from "@/features/scans/scanDetailsHelpers";
import type { ScanRun } from "@/shared/types";

import { ScanHistoryCard, ScanMonitorCard } from "./ScanMonitorCard";
import { mergeScanStatus, useScanStatuses } from "./useScanStatuses";

function isAttackScan(scan: ScanRun): boolean {
  return scan.name.startsWith("Scan (");
}

function scanDuration(scan: ScanRun): string {
  if (scan.startedAt && scan.completedAt) {
    return formatDurationMs(
      new Date(scan.completedAt).getTime() - new Date(scan.startedAt).getTime(),
    );
  }
  return "—";
}

export function ScansPage() {
  const navigate = useNavigate();
  const { scans, targets, projects, findings, loading, error, actions } = useAppStore();
  const { notify } = useToast();
  const [viewMode, setViewMode] = useViewPreference("scans");
  const [pageSize, setPageSize] = usePageSizePreference("scans");
  const [controlPending, setControlPending] = useState<string | null>(null);

  const findingsByScan = useMemo(() => {
    const map = new Map<string, number>();
    for (const finding of findings) {
      map.set(finding.scanId, (map.get(finding.scanId) ?? 0) + 1);
    }
    return map;
  }, [findings]);

  const attackScans = useMemo(
    () =>
      scans
        .filter(isAttackScan)
        .sort((a, b) => b.createdAt.localeCompare(a.createdAt)),
    [scans],
  );

  const { page, setPage, pagination } = usePaginatedList(attackScans, pageSize);

  const activeScanIds = useMemo(
    () =>
      pagination.items
        .filter(
          (scan) =>
            scan.status === "running" || scan.status === "paused" || scan.status === "pending",
        )
        .map((scan) => scan.id),
    [pagination.items],
  );

  const liveStatuses = useScanStatuses(activeScanIds, activeScanIds.length > 0);

  const targetLabel = (targetId: string | null) => {
    if (!targetId) return "—";
    const target = targets.find((item) => item.id === targetId);
    return target?.url || target?.name || "—";
  };

  const projectName = (projectId: string) =>
    projects.find((project) => project.id === projectId)?.name ?? "—";

  const runControl = useCallback(
    async (scanId: string, action: "pause" | "resume" | "stop") => {
      setControlPending(scanId);
      try {
        if (action === "pause") {
          await pauseScan(scanId);
          notify("Scan paused", "info");
        } else if (action === "resume") {
          await resumeScan(scanId);
          notify("Scan resumed", "success");
        } else {
          await stopScan(scanId);
          notify("Scan stopped", "info");
        }
        await actions.refresh();
      } catch (err) {
        const message = err instanceof Error ? err.message : "Scan control failed";
        notify(message, "error");
      } finally {
        setControlPending(null);
      }
    },
    [actions, notify],
  );

  const tableColumns = [
    {
      key: "id",
      header: "Scan ID",
      render: (scan: ScanRun) => <code className="mono text-sm">{scan.id.slice(0, 8)}…</code>,
    },
    {
      key: "project",
      header: "Project",
      render: (scan: ScanRun) => projectName(scan.projectId),
    },
    {
      key: "target",
      header: "Target",
      render: (scan: ScanRun) => <span className="mono text-sm">{targetLabel(scan.targetId)}</span>,
    },
    {
      key: "status",
      header: "Status",
      width: "110px",
      render: (scan: ScanRun) => <StatusBadge status={scan.status} />,
    },
    {
      key: "findings",
      header: "Findings",
      width: "90px",
      render: (scan: ScanRun) => findingsByScan.get(scan.id) ?? 0,
    },
    {
      key: "started",
      header: "Started",
      width: "160px",
      render: (scan: ScanRun) => formatTimestamp(scan.startedAt ?? scan.createdAt),
    },
    {
      key: "duration",
      header: "Duration",
      width: "100px",
      render: (scan: ScanRun) => scanDuration(scan),
    },
  ];

  return (
    <div className="page">
      <PageHeader
        title="Scans"
        description="Monitor background security scans"
        actions={
          <div className="page-actions">
            <RefreshButton loading={loading} onClick={() => void actions.refresh()} />
            <Link to="/scans/new">
              <Button variant="primary">New Scan</Button>
            </Link>
          </div>
        }
      />

      {error && (
        <Card>
          <p className="text-danger">{error}</p>
        </Card>
      )}

      {attackScans.length === 0 && !loading ? (
        <EmptyState
          title="No scans yet"
          description="Configure a new scan in the wizard to start a background attack job."
          action={
            <Link to="/scans/new">
              <Button variant="primary">New Scan</Button>
            </Link>
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
                onRowClick={(scan) => navigate(`/scans/${scan.id}`)}
                emptyMessage={loading ? "Loading scans…" : "No scans found"}
              />
            </Card>
          ) : (
            <div className="scan-monitor-grid">
              {pagination.items.map((scan) => {
                const live = liveStatuses.get(scan.id);
                const status = mergeScanStatus(
                  scan.id,
                  scan.status,
                  live,
                  findingsByScan.get(scan.id) ?? 0,
                );
                const isActive =
                  scan.status === "running" ||
                  scan.status === "paused" ||
                  scan.status === "pending";

                return (
                  <Link key={scan.id} to={`/scans/${scan.id}`} className="scan-card-link">
                    <Card className="scan-monitor-card-wrap">
                      {isActive ? (
                        <ScanMonitorCard
                          scan={scan}
                          status={status}
                          projectName={projectName(scan.projectId)}
                          targetName={targetLabel(scan.targetId)}
                          controlPending={controlPending === scan.id}
                          onPause={() => void runControl(scan.id, "pause")}
                          onResume={() => void runControl(scan.id, "resume")}
                          onStop={() => void runControl(scan.id, "stop")}
                        />
                      ) : (
                        <ScanHistoryCard
                          scan={scan}
                          findingsCount={findingsByScan.get(scan.id) ?? 0}
                          projectName={projectName(scan.projectId)}
                          targetName={targetLabel(scan.targetId)}
                        />
                      )}
                    </Card>
                  </Link>
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
    </div>
  );
}
