import { useCallback, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  ActionsDropdown,
  type ActionsDropdownItem,
  Button,
  Card,
  ContentToolbar,
  DataTable,
  EmptyState,
  IconPause,
  IconPlay,
  IconStop,
  IconTrash,
  PageHeader,
  Pagination,
  RefreshButton,
  scanOpenActionIcon,
  StatusBadge,
} from "@/shared/components";
import { usePageSizePreference } from "@/shared/hooks/usePageSizePreference";
import { usePaginatedList } from "@/shared/hooks/usePaginatedList";
import { useViewPreference } from "@/shared/hooks/useViewPreference";
import { pauseScan, resumeScan, stopScan, deleteScan } from "@/shared/ipc";
import { toAppError } from "@/shared/errors/AppError";
import { useToast } from "@/shared/notifications";
import { formatDurationMs, formatTimestamp } from "@/features/scans/scanDetailsHelpers";
import { isLiveScanStatus, isRetryableScanStatus, resolveScanNavigationStatus, resolveScanOpenPath, buildScanChangePlanUrl, clearWizardSessionIfReferencesScan } from "@/features/scans/wizardState";
import type { ScanRun } from "@/shared/types";

import { NewScanChooserModal } from "./NewScanChooserModal";
import { ScanHistoryCard, ScanMonitorCard } from "./ScanMonitorCard";
import { mergeScanStatus, useScanStatuses } from "./useScanStatuses";

function isAttackScan(scan: ScanRun): boolean {
  return scan.name.startsWith("Scan (") || scan.name.startsWith("Agent Scan (");
}

function isListedScan(scan: ScanRun): boolean {
  return scan.status === "draft" || isAttackScan(scan);
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
  const { scans, targets, projects, findings, ui, loading, error, actions } = useAppStore();
  const { notify, dismiss } = useToast();
  const [viewMode, setViewMode] = useViewPreference("scans");
  const [pageSize, setPageSize] = usePageSizePreference("scans");
  const [controlPending, setControlPending] = useState<string | null>(null);
  const [deletingScanId, setDeletingScanId] = useState<string | null>(null);
  const [chooserOpen, setChooserOpen] = useState(false);

  const findingsByScan = useMemo(() => {
    const map = new Map<string, number>();
    for (const finding of findings) {
      map.set(finding.scanId, (map.get(finding.scanId) ?? 0) + 1);
    }
    return map;
  }, [findings]);

  const attackScans = useMemo(
    () => {
      const query = ui.searchQuery.toLowerCase().trim();
      return scans
        .filter(isListedScan)
        .filter((scan) => {
          if (!query) return true;
          const projectName =
            projects.find((project) => project.id === scan.projectId)?.name ?? "";
          const targetName =
            targets.find((target) => target.id === scan.targetId)?.name ?? "";
          return (
            scan.name.toLowerCase().includes(query) ||
            scan.status.toLowerCase().includes(query) ||
            projectName.toLowerCase().includes(query) ||
            targetName.toLowerCase().includes(query)
          );
        })
        .sort((a, b) => b.createdAt.localeCompare(a.createdAt));
    },
    [scans, ui.searchQuery, projects, targets],
  );

  const { page, setPage, pagination } = usePaginatedList(attackScans, pageSize);

  const activeScanIds = useMemo(
    () =>
      pagination.items
        .filter((scan) => isLiveScanStatus(scan.status))
        .map((scan) => scan.id),
    [pagination.items],
  );

  const liveStatuses = useScanStatuses(activeScanIds, activeScanIds.length > 0);

  const effectiveScanStatus = useCallback(
    (scan: ScanRun) => resolveScanNavigationStatus(scan.status, liveStatuses.get(scan.id)?.status),
    [liveStatuses],
  );

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
      let pendingToastId: number | undefined;
      try {
        if (action === "pause") {
          pendingToastId = notify("Pausing scan…", "info");
          await pauseScan(scanId);
          dismiss(pendingToastId);
          pendingToastId = undefined;
          notify("Scan paused", "info");
        } else if (action === "resume") {
          pendingToastId = notify("Resuming scan…", "info");
          await resumeScan(scanId);
          dismiss(pendingToastId);
          pendingToastId = undefined;
          notify("Scan resumed", "success");
        } else {
          await stopScan(scanId);
          notify("Scan stopped", "info");
        }
        await actions.refresh();
      } catch (err) {
        if (pendingToastId !== undefined) dismiss(pendingToastId);
        const message = err instanceof Error ? err.message : "Scan control failed";
        notify(message, "error");
      } finally {
        setControlPending(null);
      }
    },
    [actions, dismiss, notify],
  );

  const openScanDetails = useCallback(
    (scan: ScanRun) => {
      navigate(`/scans/${scan.id}`);
    },
    [navigate],
  );

  const openScanAction = useCallback(
    (scan: ScanRun) => {
      navigate(resolveScanOpenPath(scan, liveStatuses.get(scan.id)?.status));
    },
    [navigate, liveStatuses],
  );

  const handleDeleteScan = useCallback(
    async (scan: ScanRun) => {
      const confirmed = window.confirm(
        `Delete scan "${scan.name}"? This permanently removes findings and reports linked to this scan.`,
      );
      if (!confirmed) return;

      setDeletingScanId(scan.id);
      try {
        await deleteScan(scan.id);
        clearWizardSessionIfReferencesScan(scan.id);
        await actions.refresh();
        notify("Scan deleted", "success");
      } catch (err) {
        notify(toAppError(err).message || "Failed to delete scan", "error");
      } finally {
        setDeletingScanId(null);
      }
    },
    [actions, notify],
  );

  const buildScanActionItems = useCallback(
    (scan: ScanRun): ActionsDropdownItem[] => {
      const openLabel =
        scan.status === "draft"
          ? "Continue Setup"
          : isLiveScanStatus(effectiveScanStatus(scan))
            ? "View Scan Progress"
            : isRetryableScanStatus(effectiveScanStatus(scan))
              ? "Retry Scan"
              : "View Scan Details";

      const items: ActionsDropdownItem[] = [
        {
          id: "open",
          label: openLabel,
          icon: scanOpenActionIcon(openLabel),
          onClick: () => openScanAction(scan),
        },
      ];

      if (isRetryableScanStatus(effectiveScanStatus(scan))) {
        items.push({
          id: "change-plan",
          label: "Change Attack Plan",
          icon: scanOpenActionIcon("Change Attack Plan"),
          onClick: () =>
            navigate(buildScanChangePlanUrl(scan.projectId, scan.id, scan.targetId)),
        });
      }

      if (scan.status === "running") {
        items.push({
          id: "pause",
          label: "Pause Scan",
          icon: <IconPause />,
          disabled: controlPending === scan.id,
          onClick: () => void runControl(scan.id, "pause"),
        });
      }
      if (scan.status === "paused") {
        items.push({
          id: "resume",
          label: "Resume Scan",
          icon: <IconPlay />,
          disabled: controlPending === scan.id,
          onClick: () => void runControl(scan.id, "resume"),
        });
      }
      if (
        scan.status === "running" ||
        scan.status === "paused" ||
        scan.status === "pending"
      ) {
        items.push({
          id: "stop",
          label: "Stop Scan",
          icon: <IconStop />,
          disabled: controlPending === scan.id,
          onClick: () => void runControl(scan.id, "stop"),
        });
      }

      items.push({
        id: "delete",
        label: "Delete Scan",
        icon: <IconTrash />,
        tone: "danger",
        disabled: deletingScanId === scan.id,
        onClick: () => void handleDeleteScan(scan),
      });

      return items;
    },
    [controlPending, deletingScanId, effectiveScanStatus, handleDeleteScan, navigate, openScanAction, runControl],
  );

  const tableColumns = useMemo(
    () => [
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
      render: (scan: ScanRun) => <StatusBadge status={effectiveScanStatus(scan)} />,
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
    {
      key: "actions",
      header: "",
      width: "56px",
      align: "right" as const,
      render: (scan: ScanRun) => (
        <span className="table-actions" onClick={(event) => event.stopPropagation()}>
          <ActionsDropdown
            label="Scan actions"
            disabled={deletingScanId === scan.id}
            items={buildScanActionItems(scan)}
          />
        </span>
      ),
    },
  ],
    [buildScanActionItems, deletingScanId, effectiveScanStatus, findingsByScan, projectName, targetLabel],
  );

  return (
    <div className="page">
      <PageHeader
        title="Scans"
        description="Track progress and results for your security test runs"
        actions={
          <div className="page-actions">
            <RefreshButton loading={loading} error={error} onClick={() => void actions.refresh()} />
            <Button variant="primary" onClick={() => setChooserOpen(true)}>
              New Scan
            </Button>
          </div>
        }
      />

      <NewScanChooserModal open={chooserOpen} onClose={() => setChooserOpen(false)} />

      {error && (
        <Card>
          <p className="text-danger">{error}</p>
        </Card>
      )}

      {attackScans.length === 0 && !loading ? (
        <EmptyState
          title="No scans yet"
          description="Use the scan wizard to configure a target and start your first test."
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
                onRowClick={openScanDetails}
                emptyMessage={loading ? "Loading scans…" : "No scans found"}
                loading={loading && pagination.items.length === 0}
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
                const isActive = isLiveScanStatus(effectiveScanStatus(scan));

                return (
                  <div
                    key={scan.id}
                    className="scan-card-link"
                    onClick={() => openScanDetails(scan)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" || event.key === " ") openScanDetails(scan);
                    }}
                    role="link"
                    tabIndex={0}
                  >
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
                          actions={
                            <ActionsDropdown
                              label="Scan actions"
                              disabled={deletingScanId === scan.id}
                              items={buildScanActionItems(scan)}
                            />
                          }
                        />
                      ) : (
                        <ScanHistoryCard
                          scan={scan}
                          findingsCount={findingsByScan.get(scan.id) ?? 0}
                          projectName={projectName(scan.projectId)}
                          targetName={targetLabel(scan.targetId)}
                          actions={
                            <ActionsDropdown
                              label="Scan actions"
                              disabled={deletingScanId === scan.id}
                              items={buildScanActionItems(scan)}
                            />
                          }
                        />
                      )}
                    </Card>
                  </div>
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
