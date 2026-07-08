import { useMemo, useState } from "react";
import { useSearchParams, useNavigate } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  Badge,
  Button,
  Card,
  ContentToolbar,
  DataTable,
  EmptyState,
  ListCard,
  PageHeader,
  Pagination,
  RefreshButton,
} from "@/shared/components";
import { usePageSizePreference } from "@/shared/hooks/usePageSizePreference";
import { usePaginatedList } from "@/shared/hooks/usePaginatedList";
import { useViewPreference } from "@/shared/hooks/useViewPreference";
import {
  buildTargetScanContext,
  formatTargetTimestamp,
} from "@/shared/targetScanContext";
import type { Project, ScanRun, Target } from "@/shared/types";

import { AddTargetModal } from "./AddTargetModal";
import { targetDisplayType } from "@/features/scans/targetProfile";

type FlatTargetRow = Target & {
  projectName: string;
  scanContext: ReturnType<typeof buildTargetScanContext>;
};

function buildFlatTargets(
  projects: Project[],
  targets: Target[],
  scans: ScanRun[],
  filterProjectId: string | null,
  query: string,
): FlatTargetRow[] {
  const normalizedQuery = query.toLowerCase().trim();

  return targets
    .filter((target) => !filterProjectId || target.projectId === filterProjectId)
    .filter((target) => {
      if (!normalizedQuery) return true;
      const projectName =
        projects.find((project) => project.id === target.projectId)?.name ?? "";
      return (
        target.name.toLowerCase().includes(normalizedQuery) ||
        target.url.toLowerCase().includes(normalizedQuery) ||
        projectName.toLowerCase().includes(normalizedQuery)
      );
    })
    .map((target) => ({
      ...target,
      projectName: projects.find((project) => project.id === target.projectId)?.name ?? "—",
      scanContext: buildTargetScanContext(target.id, scans),
    }))
    .sort((a, b) => a.projectName.localeCompare(b.projectName) || a.name.localeCompare(b.name));
}

export function TargetsPage() {
  const { targets, projects, scans, ui, loading, error, actions } = useAppStore();
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const filterProjectId = searchParams.get("projectId");
  const [modalOpen, setModalOpen] = useState(false);
  const [viewMode, setViewMode] = useViewPreference("targets");
  const [pageSize, setPageSize] = usePageSizePreference("targets");

  const rows = useMemo(
    () => buildFlatTargets(projects, targets, scans, filterProjectId, ui.searchQuery),
    [projects, targets, scans, filterProjectId, ui.searchQuery],
  );

  const { page, setPage, pagination } = usePaginatedList(rows, pageSize);

  const tableColumns = [
    {
      key: "url",
      header: "Target URL",
      render: (target: FlatTargetRow) => (
        <div>
          <strong>{target.name}</strong>
          <div className="mono text-sm text-muted">{target.url}</div>
        </div>
      ),
    },
    {
      key: "project",
      header: "Project",
      width: "150px",
      render: (target: FlatTargetRow) => target.projectName,
    },
    {
      key: "auth",
      header: "Authentication",
      width: "130px",
      render: (target: FlatTargetRow) => target.authType,
    },
    {
      key: "status",
      header: "Scan Status",
      width: "120px",
      render: (target: FlatTargetRow) => target.scanContext.scanStatusLabel,
    },
    {
      key: "lastScan",
      header: "Last Scan",
      width: "160px",
      render: (target: FlatTargetRow) => formatTargetTimestamp(target.scanContext.lastScanTime),
    },
    {
      key: "result",
      header: "Latest Result",
      render: (target: FlatTargetRow) => target.scanContext.latestScanResult,
    },
  ];

  return (
    <div className="page">
      <PageHeader
        title="Targets"
        description="Endpoints, applications, and models under test"
        actions={
          <>
            <RefreshButton loading={loading} error={error} onClick={() => void actions.refresh()} />
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

      {targets.length === 0 && !loading ? (
        <EmptyState
          title="No targets yet"
          description="Add an endpoint, application, or model to start security testing."
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
                onRowClick={(target) => navigate(`/targets/${target.id}`)}
                emptyMessage={loading ? "Loading targets…" : "No targets match your filters"}
                loading={loading && pagination.items.length === 0}
              />
            </Card>
          ) : (
            <div className="list-card-grid">
              {pagination.items.map((target) => (
                <ListCard
                  key={target.id}
                  title={target.name}
                  status={<Badge variant="info">{targetDisplayType(target)}</Badge>}
                  metadata={[
                    { label: "Project", value: target.projectName },
                    { label: "URL", value: <span className="mono text-sm">{target.url}</span> },
                    { label: "Authentication", value: target.authType },
                    { label: "Scan Status", value: target.scanContext.scanStatusLabel },
                    { label: "Latest Result", value: target.scanContext.latestScanResult },
                  ]}
                  footerMeta={`Last scan: ${formatTargetTimestamp(target.scanContext.lastScanTime)}`}
                  onClick={() => navigate(`/targets/${target.id}`)}
                />
              ))}
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

      <AddTargetModal
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        defaultProjectId={filterProjectId}
      />
    </div>
  );
}
