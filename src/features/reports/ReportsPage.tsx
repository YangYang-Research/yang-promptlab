import { useMemo } from "react";
import { useNavigate } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  Card,
  ContentToolbar,
  DataTable,
  EmptyState,
  PageHeader,
  Pagination,
  RefreshButton,
} from "@/shared/components";
import { usePageSizePreference } from "@/shared/hooks/usePageSizePreference";
import { usePaginatedList } from "@/shared/hooks/usePaginatedList";

import { buildReportScanRows, type ReportScanRow } from "./reportDownloads";

export function ReportsPage() {
  const { reports, scans, findings, projects, loading, error, actions } = useAppStore();
  const navigate = useNavigate();
  const [pageSize, setPageSize] = usePageSizePreference("reports-scans");

  const rows = useMemo(
    () =>
      buildReportScanRows({
        scans,
        findings,
        reports,
        projects,
      }),
    [scans, findings, reports, projects],
  );

  const { page, setPage, pagination } = usePaginatedList(rows, pageSize);

  const columns = [
    {
      key: "project",
      header: "Project",
      width: "160px",
      render: (row: ReportScanRow) => row.projectName,
    },
    {
      key: "scan",
      header: "Scan",
      render: (row: ReportScanRow) => (
        <div>
          <strong>{row.scanName}</strong>
          <div className="text-muted text-sm mono">{row.scanId}</div>
        </div>
      ),
    },
    {
      key: "reportId",
      header: "Report ID",
      width: "190px",
      render: (row: ReportScanRow) => <span className="mono text-sm">{row.reportId}</span>,
    },
    {
      key: "findings",
      header: "Findings",
      width: "100px",
      render: (row: ReportScanRow) => row.findingCount,
    },
    {
      key: "generated",
      header: "Last Generated",
      width: "180px",
      render: (row: ReportScanRow) => new Date(row.lastGeneratedAt).toLocaleString(),
    },
  ];

  return (
    <div className="page">
      <PageHeader
        title="Reports"
        description="Scans that have generated reports. Open one to view and export it."
        actions={<RefreshButton loading={loading} onClick={() => void actions.refresh()} />}
      />

      {error && (
        <Card>
          <p className="text-danger">{error}</p>
        </Card>
      )}

      <section className="reports-section">
        <div className="reports-section__header">
          <div>
            <h2 className="reports-section__title">Scan reports</h2>
            <span className="text-muted text-sm">
              {rows.length} scan{rows.length === 1 ? "" : "s"} with reports
            </span>
          </div>
          {rows.length > 0 && (
            <ContentToolbar
              pageSize={pageSize}
              onPageSizeChange={setPageSize}
              showViewMode={false}
            />
          )}
        </div>

        {rows.length === 0 && !loading ? (
          <Card>
            <EmptyState
              title="No reports yet"
              description="Generate a report from a completed scan to see it listed here."
            />
          </Card>
        ) : (
          <Card padding="none">
            <DataTable
              columns={columns}
              rows={pagination.items}
              keyField="scanId"
              onRowClick={(row) => navigate(`/reports/${row.reportId}`)}
              emptyMessage={loading ? "Loading reports…" : "No reports"}
              loading={loading && pagination.items.length === 0}
            />
          </Card>
        )}

        {rows.length > 0 && (
          <Pagination
            page={page}
            totalItems={pagination.totalItems}
            rangeStart={pagination.rangeStart}
            rangeEnd={pagination.rangeEnd}
            totalPages={pagination.totalPages}
            onPageChange={setPage}
          />
        )}
      </section>
    </div>
  );
}
